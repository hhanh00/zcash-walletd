use rusqlite::Connection;

pub fn create_db(connection: &Connection) -> anyhow::Result<()> {
    connection.execute(
        "CREATE TABLE IF NOT EXISTS accounts (
            id_account INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            seed TEXT,
            sk TEXT,
            ivk TEXT NOT NULL UNIQUE,
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