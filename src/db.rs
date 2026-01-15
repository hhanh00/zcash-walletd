use crate::account::{Account, AccountBalance, SubAccount};
use crate::lwd_rpc::BlockId;
use crate::network::Network;
use crate::scan::ScanEvent;
use crate::transaction::{SubAddress, Transfer};
use crate::{notify_tx, Client, Hash};
use anyhow::Result;
use sqlx::sqlite::{SqliteConnectOptions, SqliteRow};
use sqlx::{Acquire, Row, SqliteConnection, SqlitePool};
use std::collections::HashMap;
use tokio::sync::Mutex;
use tonic::Request;
use zcash_keys::address::UnifiedAddress;
use zcash_keys::encoding::AddressCodec;
use zcash_keys::keys::{UnifiedAddressRequest, UnifiedFullViewingKey};
use zcash_protocol::consensus::{NetworkUpgrade, Parameters};

pub struct Db {
    network: Network,
    pool: SqlitePool,
    ufvk: UnifiedFullViewingKey,
    notify_tx_url: String,
    address_creation_lock: Mutex<()>,
}

impl Db {
    pub async fn new(
        network: Network,
        db_path: &str,
        ufvk: &UnifiedFullViewingKey,
        notify_tx_url: &str,
    ) -> Result<Self> {
        let options = SqliteConnectOptions::new()
            .filename(db_path)
            .create_if_missing(true);
        let pool = SqlitePool::connect_with(options).await?;
        Ok(Db {
            network,
            pool,
            ufvk: ufvk.clone(),
            notify_tx_url: notify_tx_url.to_string(),
            address_creation_lock: Mutex::new(()),
        })
    }

    async fn cleanup_stale_data(connection: &mut SqliteConnection) -> Result<()> {
        sqlx::query("DELETE FROM received_notes WHERE height >=
            (SELECT MAX(height) FROM blocks)")
        .execute(connection)
        .await?;
        Ok(())
    }

    pub async fn new_account(&self, name: &str) -> Result<Account> {
        let _guard = self.address_creation_lock.lock().await;
        let mut connection = self.pool.acquire().await?;
        let (id_account,): (Option<u32>,) = sqlx::query_as("SELECT MAX(account) FROM addresses")
            .fetch_one(&mut *connection)
            .await?;
        let id_account = id_account.map(|id| id + 1).unwrap_or(0);
        let (diversifier_index, address) = self.next_diversifier(&mut connection).await?;
        self.store_receivers(
            &mut connection,
            name,
            id_account,
            0,
            diversifier_index,
            &address,
        )
        .await?;

        let account = Account {
            account_index: id_account,
            address,
        };
        Ok(account)
    }

    pub async fn new_sub_account(&self, id_account: u32, name: &str) -> Result<SubAccount> {
        let _guard = self.address_creation_lock.lock().await;
        let mut connection = self.pool.acquire().await?;
        let (id_sub_account,): (u32,) =
            sqlx::query_as("SELECT MAX(sub_account) FROM addresses WHERE account = ?1")
                .bind(id_account)
                .fetch_one(&mut *connection)
                .await?;
        let id_sub_account = id_sub_account + 1;
        let (diversifier_index, address) = self.next_diversifier(&mut connection).await?;
        self.store_receivers(
            &mut connection,
            name,
            id_account,
            id_sub_account,
            diversifier_index,
            &address,
        )
        .await?;

        let sub_account = SubAccount {
            account_index: id_account,
            sub_account_index: id_sub_account,
            address,
        };
        Ok(sub_account)
    }

    async fn store_receivers(
        &self,
        connection: &mut SqliteConnection,
        name: &str,
        id_account: u32,
        id_sub_account: u32,
        diversifier_index: u64,
        address: &str,
    ) -> Result<()> {
        let r = sqlx::query("INSERT INTO addresses(label, account, sub_account, address, diversifier_index) VALUES (?1,?2,?3,?4,?5)")
            .bind(name)
            .bind(id_account)
            .bind(id_sub_account)
            .bind(address)
            .bind(diversifier_index as i64)
            .execute(&mut *connection)
            .await?;
        let id_address = r.last_insert_rowid() as u32;

        let ua = UnifiedAddress::decode(&self.network, address).unwrap();
        if let Some(address) = ua.sapling() {
            sqlx::query(
                "INSERT INTO receivers(pool, id_address, receiver_address)
                VALUES (1, ?1, ?2)",
            )
            .bind(id_address)
            .bind(address.encode(&self.network))
            .execute(&mut *connection)
            .await?;
        }
        if let Some(address) = ua.orchard() {
            let ua = UnifiedAddress::from_receivers(Some(*address), None, None).unwrap();
            sqlx::query(
                "INSERT INTO receivers(pool, id_address, receiver_address)
                VALUES (2, ?1, ?2)",
            )
            .bind(id_address)
            .bind(ua.encode(&self.network))
            .execute(&mut *connection)
            .await?;
        }

        Ok(())
    }

    pub async fn get_accounts(
        &self,
        height: u32,
        confirmations: u32,
    ) -> Result<Vec<AccountBalance>> {
        let mut connection = self.pool.acquire().await?;
        let confirmed_height = height - confirmations + 1;
        let sub_accounts = sqlx::query(
            "WITH base AS (SELECT account, address FROM addresses WHERE sub_account = 0), \
                balances AS (SELECT account, SUM(value) AS total from received_notes WHERE spent IS NULL GROUP BY account), \
                unlocked_balances AS (SELECT account, SUM(value) AS unlocked from received_notes WHERE spent IS NULL AND height <= ?1 GROUP BY account) \
                SELECT a.account, a.label, b.total, COALESCE(u.unlocked, 0) AS unlocked, base.address as base_address \
                FROM addresses a JOIN balances b ON a.account = b.account LEFT JOIN unlocked_balances u ON u.account = a.account JOIN base ON base.account = a.account GROUP BY a.account")
            .bind(confirmed_height)
            .map(|row: SqliteRow| {
                let id_account: u32 = row.get(0);
                let label: String = row.get(1);
                let balance: u64 = row.get(2);
                let unlocked: u64 = row.get(3);
                let base_address: String = row.get(4);
                AccountBalance {
                    account_index: id_account,
                    label,
                    balance,
                    unlocked_balance: unlocked,
                    base_address,
                    tag: "".to_string(),
                }
            })
            .fetch_all(&mut *connection)
            .await?;

        Ok(sub_accounts)
    }

    pub async fn get_synced_height(&self) -> Result<u32> {
        let mut connection = self.pool.acquire().await?;
        let height = sqlx::query("SELECT MAX(height) FROM blocks")
            .map(|row: SqliteRow| {
                let h: Option<u32> = row.get(0);
                h.unwrap_or_else(|| {
                    u32::from(
                        self.network
                            .activation_height(NetworkUpgrade::Sapling)
                            .unwrap(),
                    )
                })
            })
            .fetch_one(&mut *connection)
            .await?;
        Ok(height)
    }

    pub async fn get_block_hash(&self, height: u32) -> Result<Option<[u8; 32]>> {
        let mut connection = self.pool.acquire().await?;

        let hash = sqlx::query("SELECT hash FROM blocks WHERE height = ?1")
            .bind(height)
            .map(|row: SqliteRow| {
                let hash_vec: Vec<u8> = row.get(0);
                let mut hash = [0u8; 32];
                hash.copy_from_slice(&hash_vec);
                hash
            })
            .fetch_optional(&mut *connection)
            .await?;
        Ok(hash)
    }

    fn row_to_transfer(
        row: SqliteRow,
        latest_height: u32,
        account_index: u32,
        confirmations: u32,
    ) -> Transfer {
        let address: String = row.get(0);
        let value: u64 = row.get(1);
        let sub_account: u32 = row.get(2);
        let mut txid: Vec<u8> = row.get(3);
        txid.reverse();
        let memo: String = row.get(4);
        let height: u32 = row.get(5);
        Transfer {
            address,
            amount: value,
            confirmations: latest_height - height + 1,
            height,
            fee: 0,
            note: memo,
            payment_id: "".to_string(),
            subaddr_index: SubAddress {
                major: account_index,
                minor: sub_account,
            },
            suggested_confirmations_threshold: confirmations,
            timestamp: 0, // TODO: Check if needed
            txid: hex::encode(txid),
            r#type: "in".to_string(),
            unlock_time: 0,
        }
    }

    pub async fn get_transfers(
        &self,
        latest_height: u32,
        account_index: u32,
        sub_accounts: &[u32],
        confirmations: u32,
    ) -> Result<Vec<Transfer>> {
        let mut connection = self.pool.acquire().await?;

        let transfers = sqlx::query(
            "SELECT address, n.value, sub_account, txid, memo, n.height \
            FROM received_notes n JOIN transactions t ON n.id_tx = t.id_tx WHERE \
            account = ?1 ORDER BY n.height",
        )
        .bind(account_index)
        .map(|row| Self::row_to_transfer(row, latest_height, account_index, confirmations))
        .fetch_all(&mut *connection)
        .await?;

        let transfers = transfers
            .into_iter()
            .filter(|transfer| sub_accounts.contains(&transfer.subaddr_index.minor))
            .collect::<Vec<_>>();
        Ok(transfers)
    }

    pub async fn get_transfers_by_txid(
        &self,
        latest_height: u32,
        txid: &str,
        account_index: u32,
        confirmations: u32,
    ) -> Result<Vec<Transfer>> {
        let mut connection = self.pool.acquire().await?;

        let mut txid = hex::decode(txid)?;
        txid.reverse();
        let transfers = sqlx::query(
            "SELECT a.address, n.value, n.sub_account, txid, memo, n.height
            FROM received_notes n
			JOIN transactions t ON n.id_tx = t.id_tx
			JOIN receivers r ON n.address = r.receiver_address
			JOIN addresses a ON a.id_address = r.id_address
            WHERE txid = ?1
			ORDER BY n.height",
        )
        .bind(txid)
        .map(|row| Self::row_to_transfer(row, latest_height, account_index, confirmations))
        .fetch_all(&mut *connection)
        .await?;
        Ok(transfers)
    }

    pub async fn truncate_height(&self, height: u32) -> Result<()> {
        let mut connection = self.pool.acquire().await?;

        sqlx::query("DELETE FROM transactions WHERE height >= ?1")
            .bind(height)
            .execute(&mut *connection)
            .await?;
        sqlx::query("DELETE FROM received_notes WHERE height >= ?1")
            .bind(height)
            .execute(&mut *connection)
            .await?;
        sqlx::query("DELETE FROM blocks WHERE height >= ?1")
            .bind(height)
            .execute(&mut *connection)
            .await?;
        sqlx::query("UPDATE received_notes SET spent = NULL WHERE spent >= ?1")
            .bind(height)
            .execute(&mut *connection)
            .await?;

        Ok(())
    }

    pub async fn fetch_block_hash(&self, client: &mut Client, height: u32) -> Result<()> {
        let mut connection = self.pool.acquire().await?;
        if sqlx::query("SELECT 1 FROM blocks WHERE height = ?1")
            .bind(height)
            .fetch_optional(&mut *connection)
            .await?
            .is_none()
        {
            let b = client
                .get_block(Request::new(BlockId {
                    height: height as u64,
                    hash: vec![],
                }))
                .await?
                .into_inner();
            let hash: Hash = b.hash.try_into().unwrap();
            sqlx::query(
                "INSERT INTO blocks(hash, height)
            VALUES (?1, ?2)",
            )
            .bind(hash.as_slice())
            .bind(height)
            .execute(&mut *connection)
            .await?;
        }
        Ok(())
    }

    pub async fn get_nfs(&self) -> Result<HashMap<[u8; 32], u64>> {
        let mut connection = self.pool.acquire().await?;

        let nfs = sqlx::query("SELECT nf, value FROM received_notes WHERE spent = 0")
            .map(|row: SqliteRow| {
                let nf: Vec<u8> = row.get(0);
                let value: u64 = row.get(1);
                let nf: Hash = nf.try_into().unwrap();
                (nf, value)
            })
            .fetch_all(&mut *connection)
            .await?;

        let mut nf_map = HashMap::new();
        for (nf, value) in nfs {
            nf_map.insert(nf, value);
        }
        Ok(nf_map)
    }

    async fn next_diversifier(&self, connection: &mut SqliteConnection) -> Result<(u64, String)> {
        let di = sqlx::query("SELECT MAX(diversifier_index) FROM addresses")
            .map(|r: SqliteRow| r.get::<Option<u64>, _>(0))
            .fetch_one(&mut *connection)
            .await?
            .map(|di| di + 1)
            .unwrap_or_default();
        let (ua, ndi) = self
            .ufvk
            .find_address(di.into(), UnifiedAddressRequest::AllAvailableKeys)?;
        let ua = ua.encode(&self.network);
        let ndi: u64 = ndi.try_into().unwrap();
        Ok((ndi, ua))
    }

    pub async fn create(&self) -> Result<bool> {
        let mut connection = self.pool.acquire().await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS blocks (
            height INTEGER PRIMARY KEY,
            hash BLOB NOT NULL)",
        )
        .execute(&mut *connection)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS addresses (
            id_address INTEGER PRIMARY KEY,
            label TEXT NOT NULL,
            account INTEGER NOT NULL,
            sub_account INTEGER NOT NULL,
            address TEXT NOT NULL,
            diversifier_index INTEGER NOT NULL)",
        )
        .execute(&mut *connection)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS receivers (
            id_receiver INTEGER PRIMARY KEY,
            pool INTEGER NOT NULL,
            id_address INTEGER NOT NULL,
            receiver_address TEXT NOT NULL)",
        )
        .execute(&mut *connection)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS transactions (
            id_tx INTEGER PRIMARY KEY,
            txid BLOB NOT NULL UNIQUE,
            height INTEGER NOT NULL,
            value INTEGER NOT NULL)",
        )
        .execute(&mut *connection)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS received_notes (
            id_note INTEGER PRIMARY KEY,
            address TEXT NOT NULL,
            account INTEGER,
            sub_account INTEGER,
            id_tx INTEGER NOT NULL,
            position INTEGER NOT NULL,
            height INTEGER NOT NULL,
            diversifier BLOB NOT NULL,
            value INTEGER NOT NULL,
            rcm BLOB NOT NULL,
            nf BLOB NOT NULL UNIQUE,
            rho BLOB,
            memo TEXT,
            spent INTEGER,
            CONSTRAINT tx_output UNIQUE (position))",
        )
        .execute(&mut *connection)
        .await?;

        Self::cleanup_stale_data(&mut connection).await?;

        if sqlx::query("SELECT 1 FROM pragma_table_info('received_notes') WHERE name = 'rho'")
            .fetch_optional(&mut *connection)
            .await?
            .is_none()
        {
            panic!("Old database schema. This version is not compatible with it.");
        }

        let r = sqlx::query("SELECT 1 FROM addresses")
            .map(|r: SqliteRow| r.get::<u32, _>(0))
            .fetch_optional(&mut *connection)
            .await?;

        Ok(r.is_some())
    }

    pub async fn store_events(&self, events: &[ScanEvent]) -> Result<()> {
        let mut connection = self.pool.acquire().await?;
        let mut db_transaction = connection.begin().await?;
        let db_tx = db_transaction.acquire().await?;
        let mut notify_txids = vec![];

        for event in events {
            match event {
                ScanEvent::Received(received_note) => {
                    let (id_tx, is_new) = self
                        .create_tx_if_not_exists(
                            received_note.height,
                            received_note.txid.as_slice(),
                            db_tx,
                        )
                        .await?;
                    if is_new {
                        notify_txids.push(received_note.txid);
                    }

                    let (account, sub_account) = match sqlx::query(
                        "SELECT a.account, a.sub_account FROM addresses a
                        JOIN receivers r ON a.id_address = r.id_address
                        WHERE r.receiver_address = ?1",
                    )
                    .bind(&received_note.address)
                    .map(|r: SqliteRow| {
                        let account: u32 = r.get(0);
                        let sub_account: u32 = r.get(1);
                        (account, sub_account)
                    })
                    .fetch_optional(&mut *db_tx)
                    .await?
                    {
                        Some(x) => x,
                        None => {
                            let account = sqlx::query("SELECT MAX(account) FROM addresses")
                                .map(|r: SqliteRow| {
                                    let account: Option<u32> = r.get(0);
                                    account.unwrap_or_default()
                                })
                                .fetch_one(&mut *db_tx)
                                .await?;
                            let sub_account = sqlx::query(
                                "SELECT MAX(sub_account) FROM addresses WHERE account = ?1",
                            )
                            .bind(account)
                            .map(|r: SqliteRow| {
                                let sub_account: Option<u32> = r.get(0);
                                sub_account.map(|x| x + 1).unwrap_or_default()
                            })
                            .fetch_optional(&mut *db_tx)
                            .await?
                            .unwrap_or_default();

                            let r = sqlx::query(
                                "INSERT INTO addresses
                            (label, account, sub_account, address, diversifier_index)
                            VALUES ('', ?1, ?2, ?3, ?4)",
                            )
                            .bind(account)
                            .bind(sub_account)
                            .bind(&received_note.address)
                            .bind(received_note.diversifier_index.unwrap_or_default() as u32)
                            .execute(&mut *db_tx)
                            .await?;
                            let id_address = r.last_insert_rowid() as u32;

                            sqlx::query(
                                "INSERT INTO receivers(pool, id_address, receiver_address)
                                VALUES (?1, ?2, ?3)",
                            )
                            .bind(received_note.pool)
                            .bind(id_address)
                            .bind(&received_note.address)
                            .execute(&mut *db_tx)
                            .await?;

                            (account, sub_account)
                        }
                    };

                    sqlx::query(
                        "INSERT INTO received_notes
                        (address, account, sub_account, id_tx, position, height,
                        diversifier, value, rcm, nf, rho, memo, spent)
                        VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,'',0)",
                    )
                    .bind(&received_note.address)
                    .bind(account)
                    .bind(sub_account)
                    .bind(id_tx)
                    .bind(received_note.position)
                    .bind(received_note.height)
                    .bind(received_note.diversifier.as_slice())
                    .bind(received_note.value as i64)
                    .bind(received_note.rcm.as_slice())
                    .bind(received_note.nf.as_slice())
                    .bind(received_note.rho.map(|r| r.to_vec()))
                    .execute(&mut *db_tx)
                    .await?;
                    sqlx::query("UPDATE transactions SET value = value + ?2 WHERE txid = ?1")
                        .bind(received_note.txid.as_slice())
                        .bind(received_note.value as i64)
                        .execute(&mut *db_tx)
                        .await?;
                }
                ScanEvent::Spent(spent_note) => {
                    let (_, is_new) = self.create_tx_if_not_exists(
                        spent_note.height,
                        spent_note.txid.as_slice(),
                        db_tx,
                    )
                    .await?;
                    if is_new {
                        notify_txids.push(spent_note.txid);
                    }
                    sqlx::query("UPDATE received_notes SET spent = TRUE WHERE nf = ?1")
                        .bind(spent_note.nf.as_slice())
                        .execute(&mut *db_tx)
                        .await?;
                    sqlx::query("UPDATE transactions SET value = value - ?2 WHERE txid = ?1")
                        .bind(spent_note.txid.as_slice())
                        .bind(spent_note.value as i64)
                        .execute(&mut *db_tx)
                        .await?;
                }
                ScanEvent::Memo(memo_note) => {
                    sqlx::query("UPDATE received_notes SET memo = ?2 WHERE nf = ?1")
                        .bind(memo_note.nf.as_slice())
                        .bind(&memo_note.memo)
                        .execute(&mut *db_tx)
                        .await?;
                }
                ScanEvent::Block(height, hash) => {
                    sqlx::query(
                        "INSERT INTO blocks(height, hash)
                        VALUES (?1, ?2)",
                    )
                    .bind(*height)
                    .bind(hash.as_slice())
                    .execute(&mut *db_tx)
                    .await?;
                }
            }
        }
        db_transaction.commit().await?;

        // Once committed, we can notify our listeners of the new received
        // txs
        for txid in notify_txids {
            notify_tx(&txid, &self.notify_tx_url).await?;
        }

        Ok(())
    }

    pub async fn create_tx_if_not_exists(
        &self,
        height: u32,
        txid: &[u8],
        db_tx: &mut SqliteConnection,
    ) -> Result<(u32, bool)> {
        // let txid = &received_note.txid;
        let result = match sqlx::query("SELECT id_tx FROM transactions WHERE txid = ?1")
            .bind(txid)
            .map(|r: SqliteRow| r.get::<u32, _>(0))
            .fetch_optional(&mut *db_tx)
            .await?
        {
            Some(id_tx) => (id_tx, false),
            None => {
                let r =
                    sqlx::query("INSERT INTO transactions(txid, height, value) VALUES (?1, ?2, 0)")
                        .bind(txid)
                        .bind(height)
                        .execute(db_tx)
                        .await?;
                let id_tx = r.last_insert_rowid();

                (id_tx as u32, true)
            }
        };

        Ok(result)
    }

    pub fn ufvk(&self) -> &UnifiedFullViewingKey {
        &self.ufvk
    }
}
