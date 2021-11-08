#[macro_use] extern crate rocket;

mod rpc;
mod db;
mod account;

pub use crate::rpc::*;
use zcash_primitives::consensus::Network;

pub const NETWORK: Network = Network::MainNetwork;
const DB_PATH: &str = "zec-wallet.db";

use crate::db::Db;

#[launch]
fn rocket() -> _ {
    dotenv::dotenv().unwrap();
    let db = Db::new(DB_PATH);
    db.create().unwrap();
    rocket::build()
        .manage(db)
        .mount("/", routes![
        create_account, create_address, get_accounts,
        get_transaction, make_payment,
    ])
}
