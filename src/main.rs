#[macro_use]
extern crate rocket;

#[path = "generated/cash.z.wallet.sdk.rpc.rs"]
pub mod lwd_rpc;

mod account;
mod db;
mod rpc;
mod scan;
mod transaction;

pub use crate::rpc::*;
use zcash_primitives::consensus::{Network, Parameters};

pub const NETWORK: Network = Network::MainNetwork;
const DB_PATH: &str = "zec-wallet.db";
const CONFIRMATIONS: u32 = 10;

pub const LWD_URL: &str = "http://localhost:9067";

use crate::db::Db;
use anyhow::Context;
use zcash_primitives::zip32::ExtendedFullViewingKey;
use std::sync::Mutex;
use zcash_client_backend::encoding::decode_extended_full_viewing_key;

pub struct FVK(pub Mutex<ExtendedFullViewingKey>);

#[launch]
fn rocket() -> _ {
    dotenv::dotenv().unwrap();
    let fvk = dotenv::var("VK")
        .context("Seed missing from .env file")
        .unwrap();
    let fvk = decode_extended_full_viewing_key(NETWORK.hrp_sapling_extended_full_viewing_key(), &fvk).unwrap().unwrap();
    let db = Db::new(DB_PATH, &fvk);
    let fvk = FVK(Mutex::new(fvk.clone()));
    db.create().unwrap();
    rocket::build().manage(db).manage(fvk)
        .mount(
        "/",
        routes![
            create_account,
            create_address,
            get_accounts,
            get_transaction,
            make_payment,
            request_scan,
        ],
    )
}

fn to_tonic<E: ToString>(e: E) -> tonic::Status {
    tonic::Status::internal(e.to_string())
}

fn from_tonic(e: tonic::transport::Error) -> anyhow::Error {
    anyhow::anyhow!(e.to_string())
}
