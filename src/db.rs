use rusqlite::{Connection, params};
use crate::account::{Account, derive_account};
use std::sync::{Mutex, MutexGuard};

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
            account INTEGER PRIMARY KEY NOT NULL,
            diversifier_index BLOB NOT NULL)",
            [],
        )?;

        Ok(())
    }
}