#[macro_use] extern crate rocket;

mod rpc;
mod db;
mod account;
mod transaction;

pub use crate::rpc::*;
use zcash_primitives::consensus::Network;

pub const NETWORK: Network = Network::MainNetwork;
const DB_PATH: &str = "zec-wallet.db";

use crate::db::Db;
use anyhow::Context;

#[launch]
fn rocket() -> _ {
    dotenv::dotenv().unwrap();
    let fvk = dotenv::var("VK").context("Seed missing from .env file").unwrap();
    let db = Db::new(DB_PATH, &fvk);
    db.create().unwrap();
    rocket::build()
        .manage(db)
        .mount("/", routes![
        create_account, create_address, get_accounts,
        get_transaction, make_payment,
    ])
}
