use crate::account::{Account, AccountBalance, SubAccount};
use crate::network::Network;
use crate::scan::DecryptedNote;
use crate::transaction::{SubAddress, Transfer};
use anyhow::Result;
use sapling_crypto::zip32::ExtendedFullViewingKey;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqliteConnection, SqlitePool};
use std::collections::HashMap;
use zcash_client_backend::encoding::encode_payment_address;
use zcash_primitives::consensus::{NetworkConstants as _, NetworkUpgrade, Parameters};
use zcash_primitives::zip32::DiversifierIndex;

pub struct Db {
    network: Network,
    pool: SqlitePool,
    fvk: ExtendedFullViewingKey,
}

impl Db {
    pub async fn new(
        network: Network,
        db_path: &str,
        fvk: &ExtendedFullViewingKey,
    ) -> Result<Self> {
        let pool = SqlitePool::connect(db_path).await?;
        Ok(Db {
            network,
            pool,
            fvk: fvk.clone(),
        })
    }

    pub async fn new_account(&self, name: &str) -> Result<Account> {
        let mut connection = self.pool.acquire().await?;
        let (id_account,): (Option<u32>,) = sqlx::query_as("SELECT MAX(account) FROM addresses")
            .fetch_one(&mut *connection)
            .await?;
        let id_account = id_account.map(|id| id + 1).unwrap_or(0);
        let (diversifier_index, address) = self.next_diversifier(&mut connection).await?;

        sqlx::query("INSERT INTO addresses(label, account, 0, address, diversifier_index) VALUES (?1,?2,0,?3,?4)")
            .bind(name)
            .bind(id_account)
            .bind(&address)
            .bind(diversifier_index as i64)
            .execute(&mut *connection)
            .await?;
        let account = Account {
            account_index: id_account,
            address,
        };
        Ok(account)
    }

    pub async fn new_sub_account(&self, id_account: u32, name: &str) -> Result<SubAccount> {
        let mut connection = self.pool.acquire().await?;
        let (id_sub_account,): (u32,) =
            sqlx::query_as("SELECT MAX(sub_account) FROM addresses WHERE account = ?1")
                .bind(id_account)
                .fetch_one(&mut *connection)
                .await?;
        let id_sub_account = id_sub_account + 1;
        let (diversifier_index, address) = self.next_diversifier(&mut connection).await?;
        sqlx::query("INSERT INTO addresses(label, account, sub_account, address, diversifier_index) VALUES (?1,?2,?3,?4,?5)")
            .bind(name)
            .bind(id_account)
            .bind(id_sub_account)
            .bind(&address)
            .bind(diversifier_index as i64)
            .execute(&mut *connection)
            .await?;

        let sub_account = SubAccount {
            account_index: id_account,
            sub_account_index: id_sub_account,
            address,
        };

        Ok(sub_account)
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

    pub async fn store_note(&self, note: &DecryptedNote, id_tx: u32) -> Result<u32> {
        let mut connection = self.pool.acquire().await?;
        let r = sqlx::query("SELECT account, sub_account FROM addresses WHERE address = ?1")
            .bind(&note.address)
            .map(|row: SqliteRow| {
                let account: u32 = row.get(0);
                let sub_account: u32 = row.get(1);
                (account, sub_account)
            })
            .fetch_optional(&mut *connection)
            .await?;
        let (account, sub_account) = match r {
            Some((a, s)) => (Some(a), Some(s)),
            None => (None, None),
        };

        let r = sqlx::query(
            "INSERT INTO received_notes(id_tx, address, position, height, diversifier, value, rcm, nf, memo, account, sub_account) \
            SELECT ?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11")
            .bind(id_tx)
            .bind(&note.address)
            .bind(note.position as u32)
            .bind(note.height)
            .bind(note.diversifier.to_vec())
            .bind(note.value as i64)
            .bind(note.rcm.to_vec())
            .bind(note.nf.to_vec())
            .bind(&note.memo)
            .bind(account)
            .bind(sub_account)
            .execute(&mut *connection)
            .await?;
        let id_note = r.last_insert_rowid();
        Ok(id_note as u32)
    }

    pub async fn store_tx(&self, txid: &[u8], height: u32, value: i64) -> Result<u32> {
        let mut connection = self.pool.acquire().await?;
        let r = sqlx::query("INSERT INTO transactions(txid, height, value) VALUES (?1,?2,?3)")
            .bind(txid)
            .bind(height)
            .bind(value)
            .execute(&mut *connection)
            .await?;
        let id_tx = r.last_insert_rowid() as u32;
        Ok(id_tx)
    }

    pub async fn store_block(&self, height: u32, hash: &[u8]) -> Result<()> {
        let mut connection = self.pool.acquire().await?;
        sqlx::query("INSERT INTO blocks(height, hash) VALUES (?1,?2)")
            .bind(height)
            .bind(hash)
            .execute(&mut *connection)
            .await?;
        Ok(())
    }

    pub async fn get_synced_height(&self) -> Result<u32> {
        let mut connection = self.pool.acquire().await?;
        let height = sqlx::query("SELECT MAX(height) FROM blocks")
            .map(|row: SqliteRow| {
                let h: Option<u32> = row.get(0);
                let height = h.unwrap_or_else(|| {
                    u32::from(
                        self.network
                            .activation_height(NetworkUpgrade::Sapling)
                            .unwrap(),
                    )
                });
                height
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

    pub async fn mark_spent(&self, id_note: u32, id_tx: u32) -> Result<()> {
        let mut connection = self.pool.acquire().await?;

        sqlx::query("UPDATE received_notes SET spent = ?1 WHERE id_note = ?2")
            .bind(id_tx)
            .bind(id_note)
            .execute(&mut *connection)
            .await?;
        Ok(())
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
        let t = Transfer {
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
        };
        t
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
            account = ?1",
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
            "SELECT address, n.value, sub_account, txid, memo, n.height \
            FROM received_notes n JOIN transactions t ON n.id_tx = t.id_tx WHERE \
            txid = ?1",
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

    pub async fn get_nfs(&self) -> Result<HashMap<[u8; 32], u32>> {
        let mut connection = self.pool.acquire().await?;

        let nfs = sqlx::query("SELECT id_note, nf FROM received_notes WHERE spent IS NULL")
            .map(|row: SqliteRow| {
                let id_note: u32 = row.get(0);
                let nf: Vec<u8> = row.get(1);
                let mut nf_bytes = [0u8; 32];
                nf_bytes.copy_from_slice(&nf);
                (id_note, nf_bytes)
            })
            .fetch_all(&mut *connection)
            .await?;

        let mut nf_map = HashMap::<[u8; 32], u32>::new();
        for (id_note, nf) in nfs {
            nf_map.insert(nf, id_note);
        }
        Ok(nf_map)
    }

    async fn next_diversifier(&self, connection: &mut SqliteConnection) -> Result<(u64, String)> {
        let (diversifier,): (Option<u64>,) =
            sqlx::query_as("SELECT MAX(diversifier_index) FROM addresses")
                .fetch_one(&mut *connection)
                .await?;
        let (next_index, pa) = if let Some(diversifier) = diversifier {
            let mut di = [0u8; 11];
            di[0..8].copy_from_slice(&(diversifier + 1).to_le_bytes());
            let index = DiversifierIndex::from(di);
            self.fvk
                .find_address(index)
                .ok_or_else(|| anyhow::anyhow!("Could not derive new subaccount"))?
        } else {
            self.fvk.default_address()
        };
        let mut di = [0u8; 8];
        di.copy_from_slice(&next_index.as_bytes()[0..8]);
        let next_index = u64::from_le_bytes(di);
        Ok((
            next_index,
            encode_payment_address(self.network.hrp_sapling_payment_address(), &pa),
        ))
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
            memo TEXT,
            spent INTEGER,
            CONSTRAINT tx_output UNIQUE (position))",
        )
        .execute(&mut *connection)
        .await?;

        let r = sqlx::query("SELECT 1 FROM addresses")
            .map(|r: SqliteRow| r.get::<u32, _>(0))
            .fetch_optional(&mut *connection)
            .await?;

        Ok(r.is_some())
    }
}
