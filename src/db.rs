use rusqlite::{Connection, params};
use crate::account::{Account, AccountBalance, SubAccount};
use std::sync::{Mutex, MutexGuard};
use zcash_client_backend::encoding::{decode_extended_full_viewing_key, encode_payment_address};
use crate::NETWORK;
use zcash_primitives::consensus::Parameters;
use zcash_primitives::zip32::{DiversifierIndex, ExtendedFullViewingKey};

pub struct Db {
    connection: Mutex<Connection>,
    fvk: ExtendedFullViewingKey,
}

impl Db {
    pub fn new(db_path: &str, fvk: &str) -> Self {
        let fvk = decode_extended_full_viewing_key(NETWORK.hrp_sapling_extended_full_viewing_key(), fvk).unwrap().unwrap();
        Db {
            connection: Mutex::new(Connection::open(db_path).unwrap()),
            fvk
        }
    }

    fn grab_lock(&self) -> MutexGuard<Connection> { self.connection.lock().unwrap() }

    pub fn new_account(&self, name: &str) -> anyhow::Result<Account> {
        let connection = self.grab_lock();
        let id_account: Option<u32> = connection.query_row("SELECT MAX(account) FROM addresses", [], |row| row.get(0))?;
        let id_account = id_account.map(|id| id+1).unwrap_or(0);
        let (diversifier_index, address) = self.next_diversifier(&connection)?;

        connection.execute("INSERT INTO addresses(label, account, sub_account, address, diversifier_index) VALUES (?1,?2,?3,?4,?5)",
                           params![name, id_account, 0, &address, diversifier_index])?;
        connection.execute("INSERT INTO balances(account, total, unlocked) VALUES (?1,0,0)",
                           params![id_account])?;
        let account = Account {
            account_index: id_account,
            address
        };
        Ok(account)
    }

    pub fn new_sub_account(&self, id_account: u32, name: &str) -> anyhow::Result<SubAccount> {
        let connection = self.grab_lock();
        let id_sub_account: u32 = connection.query_row("SELECT MAX(sub_account) FROM addresses WHERE account = ?1",
                                                           params![id_account],
                                                           |row|
                                                               row.get(0))?;
        let id_sub_account = id_sub_account + 1;
        let (diversifier_index, address) = self.next_diversifier(&connection)?;
        connection.execute("INSERT INTO addresses(label, account, sub_account, address, diversifier_index) VALUES (?1,?2,?3,?4,?5)",
                           params![name, id_account, id_sub_account, &address, diversifier_index])?;

        let sub_account = SubAccount {
            account_index: id_account,
            sub_account_index: id_sub_account,
            address
        };

        Ok(sub_account)
    }

    pub fn get_accounts(&self) -> anyhow::Result<Vec<AccountBalance>> {
        let connection = self.grab_lock();

        // TODO: Group balance by sub account
        let mut s = connection.prepare(
            "SELECT a.account, total, address, label, unlocked FROM addresses a JOIN balances b ON a.account = b.account")?;
        let rows = s.query_map([], |row| {
            let id_account: u32 = row.get(0)?;
            let total: u64 = row.get(1)?;
            let address: String = row.get(2)?;
            let name: String = row.get(3)?;
            let unlocked: u64 = row.get(4)?;
            Ok(AccountBalance {
                account_index: id_account,
                balance: total,
                base_address: address.clone(),
                label: name.clone(),
                tag: "".to_string(),
                unlocked_balance: unlocked,
            })
        })?;

        let mut sub_accounts: Vec<AccountBalance> = vec![];
        for row in rows {
            let sa = row?;
            sub_accounts.push(sa);
        }

        Ok(sub_accounts)
    }

    fn next_diversifier(&self, connection: &Connection) -> anyhow::Result<(u64, String)> {
        let diversifier: Option<u64> = connection.query_row("SELECT MAX(diversifier_index) FROM addresses", [], |row| row.get(0))?;
        let (next_index, pa) = if let Some(diversifier) = diversifier {
            let mut di = [0u8; 11];
            di[0..8].copy_from_slice(&diversifier.to_le_bytes());
            let mut index = DiversifierIndex(di);
            index.increment().map_err(|_| anyhow::anyhow!("Out of diversified addresses"))?;
            let (index, pa) = self.fvk.address(index).map_err(|_| anyhow::anyhow!("Could not derive new subaccount"))?;
            (index, pa)
        } else {
            self.fvk.default_address().map_err(|_| anyhow::anyhow!("Cannot get default address"))?
        };
        let mut di = [0u8; 8];
        di.copy_from_slice(&next_index.0[0..8]);
        let next_index = u64::from_le_bytes(di);
        Ok((next_index, encode_payment_address(NETWORK.hrp_sapling_payment_address(), &pa)))
    }

    pub fn create(&self) -> anyhow::Result<()> {
        let connection = self.grab_lock();
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
            "CREATE TABLE IF NOT EXISTS balances (
            account INTEGER PRIMARY KEY,
            total INTEGER NOT NULL,
            unlocked INTEGER NOT NULL)",
            [],
        )?;

        connection.execute(
            "CREATE TABLE IF NOT EXISTS transactions (
            id_tx INTEGER PRIMARY KEY,
            account INTEGER NOT NULL,
            txid BLOB NOT NULL,
            height INTEGER NOT NULL,
            timestamp INTEGER NOT NULL,
            value INTEGER NOT NULL,
            address TEXT,
            memo TEXT,
            tx_index INTEGER,
            CONSTRAINT tx_account UNIQUE (height, tx_index))",
            [],
        )?;

        connection.execute(
            "CREATE TABLE IF NOT EXISTS transfers (
            id_tx INTEGER NOT NULL,
            id_note INTEGER NOT NULL,
            is_spent BOOL NOT NULL,
            PRIMARY KEY (id_tx, id_note))",
            [],
        )?;

        connection.execute(
            "CREATE TABLE IF NOT EXISTS received_notes (
            id_note INTEGER PRIMARY KEY,
            position INTEGER NOT NULL,
            height INTEGER NOT NULL,
            output_index INTEGER NOT NULL,
            diversifier BLOB NOT NULL,
            value INTEGER NOT NULL,
            rcm BLOB NOT NULL,
            nf BLOB NOT NULL UNIQUE,
            memo TEXT,
            spent INTEGER,
            CONSTRAINT tx_output UNIQUE (position))",
            [],
        )?;

        Ok(())
    }
}