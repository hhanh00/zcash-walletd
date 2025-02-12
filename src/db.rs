use crate::account::{Account, AccountBalance, SubAccount};
use crate::network::Network;
use crate::scan::DecryptedNote;
use crate::transaction::{SubAddress, Transfer};
use rusqlite::{params, Connection, OptionalExtension, Row};
use sapling_crypto::zip32::ExtendedFullViewingKey;
use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};
use zcash_client_backend::encoding::encode_payment_address;
use zcash_primitives::consensus::{NetworkConstants as _, NetworkUpgrade, Parameters};
use zcash_primitives::zip32::DiversifierIndex;

pub struct Db {
    network: Network,
    connection: Mutex<Connection>,
    fvk: ExtendedFullViewingKey,
}

impl Db {
    pub fn new(network: Network, db_path: &str, fvk: &ExtendedFullViewingKey) -> Self {
        Db {
            network,
            connection: Mutex::new(Connection::open(db_path).unwrap()),
            fvk: fvk.clone(),
        }
    }

    fn grab_lock(&self) -> MutexGuard<Connection> {
        self.connection.lock().unwrap()
    }

    pub fn new_account(&self, name: &str) -> anyhow::Result<Account> {
        let connection = self.grab_lock();
        let id_account: Option<u32> =
            connection.query_row("SELECT MAX(account) FROM addresses", [], |row| row.get(0))?;
        let id_account = id_account.map(|id| id + 1).unwrap_or(0);
        let (diversifier_index, address) = self.next_diversifier(&connection)?;

        connection.execute("INSERT INTO addresses(label, account, sub_account, address, diversifier_index) VALUES (?1,?2,?3,?4,?5)",
                           params![name, id_account, 0, &address, diversifier_index])?;
        let account = Account {
            account_index: id_account,
            address,
        };
        Ok(account)
    }

    pub fn new_sub_account(&self, id_account: u32, name: &str) -> anyhow::Result<SubAccount> {
        let connection = self.grab_lock();
        let id_sub_account: u32 = connection.query_row(
            "SELECT MAX(sub_account) FROM addresses WHERE account = ?1",
            params![id_account],
            |row| row.get(0),
        )?;
        let id_sub_account = id_sub_account + 1;
        let (diversifier_index, address) = self.next_diversifier(&connection)?;
        connection.execute("INSERT INTO addresses(label, account, sub_account, address, diversifier_index) VALUES (?1,?2,?3,?4,?5)",
                           params![name, id_account, id_sub_account, &address, diversifier_index])?;

        let sub_account = SubAccount {
            account_index: id_account,
            sub_account_index: id_sub_account,
            address,
        };

        Ok(sub_account)
    }

    pub fn get_accounts(
        &self,
        height: u32,
        confirmations: u32,
    ) -> anyhow::Result<Vec<AccountBalance>> {
        let connection = self.grab_lock();
        let mut s = connection.prepare(
            "WITH base AS (SELECT account, address FROM addresses WHERE sub_account = 0), \
            balances AS (SELECT account, SUM(value) AS total from received_notes WHERE spent IS NULL GROUP BY account), \
            unlocked_balances AS (SELECT account, SUM(value) AS unlocked from received_notes WHERE spent IS NULL AND height <= ?1 GROUP BY account) \
            SELECT a.account, a.label, b.total, COALESCE(u.unlocked, 0) AS unlocked, base.address as base_address \
            FROM addresses a JOIN balances b ON a.account = b.account LEFT JOIN unlocked_balances u ON u.account = a.account JOIN base ON base.account = a.account GROUP BY a.account")?;

        let confirmed_height = height - confirmations + 1;
        let rows = s.query_map([confirmed_height], |row| {
            let id_account: u32 = row.get(0)?;
            let label: String = row.get(1)?;
            let balance: u64 = row.get(2)?;
            let unlocked: u64 = row.get(3)?;
            let base_address: String = row.get(4)?;
            Ok(AccountBalance {
                account_index: id_account,
                label,
                balance,
                unlocked_balance: unlocked,
                base_address,
                tag: "".to_string(),
            })
        })?;

        let mut sub_accounts: Vec<AccountBalance> = vec![];
        for row in rows {
            let sa = row?;
            sub_accounts.push(sa);
        }

        Ok(sub_accounts)
    }

    pub fn store_note(&self, note: &DecryptedNote, id_tx: u32) -> anyhow::Result<u32> {
        let connection = self.grab_lock();

        let r = connection
            .query_row(
                "SELECT account, sub_account FROM addresses WHERE address = ?1",
                [&note.address],
                |row| {
                    let account: u32 = row.get(0)?;
                    let sub_account: u32 = row.get(1)?;
                    Ok((account, sub_account))
                },
            )
            .optional()?;
        let (account, sub_account) = match r {
            Some((a, s)) => (Some(a), Some(s)),
            None => (None, None),
        };

        connection.execute(
            "INSERT INTO received_notes(id_tx, address, position, height, diversifier, value, rcm, nf, memo, account, sub_account) \
            SELECT ?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11",
            params![id_tx, &note.address, note.position, note.height, note.diversifier.to_vec(), note.value, note.rcm.to_vec(), note.nf.to_vec(), note.memo,
            account, sub_account])?;
        let id_note = connection.last_insert_rowid();
        Ok(id_note as u32)
    }

    pub fn store_tx(&self, txid: &[u8], height: u32, value: i64) -> anyhow::Result<u32> {
        let connection = self.grab_lock();

        connection.execute(
            "INSERT INTO transactions(txid, height, value) VALUES (?1,?2,?3)",
            params![txid, height, value],
        )?;
        let id_tx = connection.last_insert_rowid() as u32;
        Ok(id_tx)
    }

    pub fn store_block(&self, height: u32, hash: &[u8]) -> anyhow::Result<()> {
        let connection = self.grab_lock();

        connection.execute(
            "INSERT INTO blocks(height, hash) VALUES (?1,?2)",
            params![height, hash],
        )?;
        Ok(())
    }

    pub fn get_synced_height(&self) -> anyhow::Result<u32> {
        let connection = self.grab_lock();

        let height = connection.query_row("SELECT MAX(height) FROM blocks", [], |row| {
            let h: Option<u32> = row.get(0)?;
            let height = h.unwrap_or_else(|| {
                u32::from(
                    self.network
                        .activation_height(NetworkUpgrade::Sapling)
                        .unwrap(),
                )
            });
            Ok(height)
        })?;
        Ok(height)
    }

    pub fn get_block_hash(&self, height: u32) -> anyhow::Result<Option<[u8; 32]>> {
        let connection = self.grab_lock();

        let hash = connection
            .query_row(
                "SELECT hash FROM blocks WHERE height = ?1",
                [height],
                |row| {
                    let hash_vec: Vec<u8> = row.get(0)?;
                    let mut hash = [0u8; 32];
                    hash.copy_from_slice(&hash_vec);
                    Ok(hash)
                },
            )
            .optional()?;
        Ok(hash)
    }

    pub fn mark_spent(&self, id_note: u32, id_tx: u32) -> anyhow::Result<()> {
        let connection = self.grab_lock();

        connection.execute(
            "UPDATE received_notes SET spent = ?1 WHERE id_note = ?2",
            params![id_tx, id_note],
        )?;
        Ok(())
    }

    fn row_to_transfer(
        row: &Row,
        latest_height: u32,
        account_index: u32,
        confirmations: u32,
    ) -> rusqlite::Result<Transfer> {
        let address: String = row.get(0)?;
        let value: u64 = row.get(1)?;
        let sub_account: u32 = row.get(2)?;
        let mut txid: Vec<u8> = row.get(3)?;
        txid.reverse();
        let memo: String = row.get(4)?;
        let height: u32 = row.get(5)?;
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
        Ok(t)
    }

    pub fn get_transfers(
        &self,
        latest_height: u32,
        account_index: u32,
        sub_accounts: &[u32],
        confirmations: u32,
    ) -> anyhow::Result<Vec<Transfer>> {
        let connection = self.grab_lock();

        let mut s = connection.prepare(
            "SELECT address, n.value, sub_account, txid, memo, n.height \
        FROM received_notes n JOIN transactions t ON n.id_tx = t.id_tx WHERE \
        account = ?1",
        )?;
        let rows = s.query_map([account_index], |row| {
            Self::row_to_transfer(row, latest_height, account_index, confirmations)
        })?;
        let mut transfers: Vec<Transfer> = vec![];
        for row in rows {
            let row = row?;
            if sub_accounts.contains(&row.subaddr_index.minor) {
                transfers.push(row);
            }
        }
        Ok(transfers)
    }

    pub fn get_transfers_by_txid(
        &self,
        latest_height: u32,
        txid: &str,
        account_index: u32,
        confirmations: u32,
    ) -> anyhow::Result<Vec<Transfer>> {
        let connection = self.grab_lock();

        let mut txid = hex::decode(txid)?;
        txid.reverse();
        let mut s = connection.prepare(
            "SELECT address, n.value, sub_account, txid, memo, n.height \
        FROM received_notes n JOIN transactions t ON n.id_tx = t.id_tx WHERE \
        txid = ?1",
        )?;
        let rows = s.query_map(params![&txid], |row| {
            Self::row_to_transfer(row, latest_height, account_index, confirmations)
        })?;
        let mut transfers: Vec<Transfer> = vec![];
        for row in rows {
            let row = row?;
            transfers.push(row);
        }
        Ok(transfers)
    }

    pub fn truncate_height(&self, height: u32) -> anyhow::Result<()> {
        let connection = self.grab_lock();

        connection.execute("DELETE FROM transactions WHERE height >= ?1", [height])?;
        connection.execute("DELETE FROM received_notes WHERE height >= ?1", [height])?;
        connection.execute("DELETE FROM blocks WHERE height >= ?1", [height])?;
        connection.execute(
            "UPDATE received_notes SET spent = NULL WHERE spent >= ?1",
            [height],
        )?;

        Ok(())
    }

    pub fn get_nfs(&self) -> anyhow::Result<HashMap<[u8; 32], u32>> {
        let connection = self.grab_lock();

        let mut s =
            connection.prepare("SELECT id_note, nf FROM received_notes WHERE spent IS NULL")?;
        let nfs = s.query_map([], |row| {
            let id_note: u32 = row.get(0)?;
            let nf: Vec<u8> = row.get(1)?;
            let mut nf_bytes = [0u8; 32];
            nf_bytes.copy_from_slice(&nf);
            Ok((id_note, nf_bytes))
        })?;
        let mut nf_map = HashMap::<[u8; 32], u32>::new();
        for nf in nfs {
            let (id_note, nf) = nf?;
            nf_map.insert(nf, id_note);
        }
        Ok(nf_map)
    }

    fn next_diversifier(&self, connection: &Connection) -> anyhow::Result<(u64, String)> {
        let diversifier: Option<u64> =
            connection.query_row("SELECT MAX(diversifier_index) FROM addresses", [], |row| {
                row.get(0)
            })?;
        let (next_index, pa) = if let Some(diversifier) = diversifier {
            let mut di = [0u8; 11];
            di[0..8].copy_from_slice(&diversifier.to_le_bytes());
            let mut index = DiversifierIndex::from(di);
            index
                .increment()
                .map_err(|_| anyhow::anyhow!("Out of diversified addresses"))?;
            let pa = self
                .fvk
                .address(index)
                .ok_or_else(|| anyhow::anyhow!("Could not derive new subaccount"))?;
            (index, pa)
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

    pub fn create(&self) -> anyhow::Result<bool> {
        let connection = self.grab_lock();

        connection.execute(
            "CREATE TABLE IF NOT EXISTS blocks (
            height INTEGER PRIMARY KEY,
            hash BLOB NOT NULL)",
            [],
        )?;

        connection.execute(
            "CREATE TABLE IF NOT EXISTS addresses (
            id_address INTEGER PRIMARY KEY,
            label TEXT NOT NULL,
            account INTEGER NOT NULL,
            sub_account INTEGER NOT NULL,
            address TEXT NOT NULL,
            diversifier_index INTEGER NOT NULL)",
            [],
        )?;

        connection.execute(
            "CREATE TABLE IF NOT EXISTS transactions (
            id_tx INTEGER PRIMARY KEY,
            txid BLOB NOT NULL UNIQUE,
            height INTEGER NOT NULL,
            value INTEGER NOT NULL)",
            [],
        )?;

        connection.execute(
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
            [],
        )?;

        let r: Option<u32> = connection
            .query_row("SELECT 1 FROM addresses", [], |r| r.get(0))
            .optional()?;

        Ok(r.is_some())
    }
}
