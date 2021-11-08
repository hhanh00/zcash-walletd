use rusqlite::{Connection, params, OptionalExtension};
use crate::account::{Account, derive_account, DiversifiedAddress};
use std::sync::{Mutex, MutexGuard};
use zcash_client_backend::encoding::{decode_extended_full_viewing_key, encode_payment_address};
use crate::NETWORK;
use zcash_primitives::consensus::Parameters;
use zcash_primitives::zip32::DiversifierIndex;

pub struct Db {
    connection: Mutex<Connection>,
}

impl Db {
    pub fn new(db_path: &str) -> Self {
        Db {
            connection: Mutex::new(Connection::open(db_path).unwrap())
        }
    }

    fn grab_lock(&self) -> MutexGuard<Connection> { self.connection.lock().unwrap() }

    pub fn derive_account(&self, name: &str, seed: &str) -> anyhow::Result<Account> {
        let connection = self.grab_lock();
        connection.execute("INSERT INTO accounts(name, sk, ivk, address) VALUES (?1,?2,?3,?4)",
                           [name, "", "", ""])?;
        let account_id = connection.last_insert_rowid() as u32;
        let account = derive_account(seed, account_id)?;
        connection.execute("UPDATE accounts SET sk=?1, ivk=?2, address=?3 WHERE id_account=?4",
                           params![&account.esk, &account.efvk, &account.address, account_id])?;
        Ok(account)
    }

    pub fn new_diversified_address(&self, account: u32, name: &str) -> anyhow::Result<DiversifiedAddress> {
        let connection = self.grab_lock();
        let efvk: String = connection.query_row("SELECT ivk FROM accounts WHERE id_account = ?1",
                                                params![account],
                                                |row|
                                                    row.get(0))?;
        let next_index: Option<i64> = connection.query_row("SELECT MAX(diversifier_index) FROM diversifiers WHERE account = ?1",
                                                           params![account],
                                                           |row|
                                                               row.get(0))?;
        let next_index = next_index.unwrap_or(1);
        let fvk = decode_extended_full_viewing_key(NETWORK.hrp_sapling_extended_full_viewing_key(), &efvk)?.unwrap();
        let mut di = [0u8; 11];
        di[0..8].copy_from_slice(&next_index.to_le_bytes());
        let (mut index, pa) = fvk.address(DiversifierIndex(di)).map_err(|_| anyhow::anyhow!("Could not derive new subaccount"))?;
        index.increment().map_err(|_| anyhow::anyhow!("Out of diversified addresses"))?;
        let mut di = [0u8; 8];
        di.copy_from_slice(&index.0[0..8]);
        let next_index = i64::from_le_bytes(di);
        let address = encode_payment_address(NETWORK.hrp_sapling_payment_address(), &pa);
        connection.execute("INSERT INTO diversifiers(account, name, diversifier_index, address) VALUES (?1,?2,?3,?4)",
                           params![account, name, next_index, address])?;

        let address = DiversifiedAddress {
            address: address.clone(),
            index: next_index,
        };

        Ok(address)
    }

    pub fn create(&self) -> anyhow::Result<()> {
        let connection = self.grab_lock();
        connection.execute(
            "CREATE TABLE IF NOT EXISTS accounts (
            id_account INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            sk TEXT NOT NULL,
            ivk TEXT NOT NULL,
            address TEXT NOT NULL)",
            [],
        )?;

        connection.execute(
            "CREATE TABLE IF NOT EXISTS blocks (
            height INTEGER PRIMARY KEY,
            hash BLOB NOT NULL,
            timestamp INTEGER NOT NULL,
            sapling_tree BLOB NOT NULL)",
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
            CONSTRAINT tx_account UNIQUE (height, tx_index, account))",
            [],
        )?;

        connection.execute(
            "CREATE TABLE IF NOT EXISTS received_notes (
            id_note INTEGER PRIMARY KEY,
            account INTEGER NOT NULL,
            position INTEGER NOT NULL,
            tx INTEGER NOT NULL,
            height INTEGER NOT NULL,
            output_index INTEGER NOT NULL,
            diversifier BLOB NOT NULL,
            value INTEGER NOT NULL,
            rcm BLOB NOT NULL,
            nf BLOB NOT NULL UNIQUE,
            spent INTEGER,
            CONSTRAINT tx_output UNIQUE (tx, output_index))",
            [],
        )?;

        connection.execute(
            "CREATE TABLE IF NOT EXISTS sapling_witnesses (
            id_witness INTEGER PRIMARY KEY,
            note INTEGER NOT NULL,
            height INTEGER NOT NULL,
            witness BLOB NOT NULL,
            CONSTRAINT witness_height UNIQUE (note, height))",
            [],
        )?;

        connection.execute(
            "CREATE TABLE IF NOT EXISTS diversifiers (
            id_diversifier INTEGER PRIMARY KEY NOT NULL,
            account INTEGER NOT NULL,
            name TEXT NOT NULL,
            diversifier_index BLOB NOT NULL,
            address TEXT NOT NULL)",
            [],
        )?;

        Ok(())
    }
}