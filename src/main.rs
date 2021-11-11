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

pub const NETWORK: Network = Network::YCashMainNetwork;

// They come from the config file
//
// const DB_PATH: &str = "zec-wallet.db";
// const CONFIRMATIONS: u32 = 10;
//
// pub const LWD_URL: &str = "https://lite.ycash.xyz:9067";
// pub const NOTIFY_TX_URL: &str = "https://localhost:14142/zcashlikedaemoncallback/tx?cryptoCode=yec&hash=";

use crate::db::Db;
use anyhow::Context;
use zcash_primitives::zip32::ExtendedFullViewingKey;
use std::sync::Mutex;
use zcash_client_backend::encoding::decode_extended_full_viewing_key;
use crate::scan::monitor_task;
use rocket::fairing::AdHoc;
use serde::Deserialize;

pub struct FVK(pub Mutex<ExtendedFullViewingKey>);

#[derive(Deserialize)]
pub struct WalletConfig {
    port: u16,
    db_path: String,
    confirmations: u32,
    lwd_url: String,
    notify_tx_url: String,
}

#[rocket::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().unwrap();
    env_logger::init();
    let fvk = dotenv::var("VK")
        .context("Seed missing from .env file")
        .unwrap();
    let fvk = decode_extended_full_viewing_key(NETWORK.hrp_sapling_extended_full_viewing_key(), &fvk).unwrap().unwrap();
    let rocket = rocket::build();
    let figment = rocket.figment();
    let config: WalletConfig = figment.extract().unwrap();
    let db = Db::new(&config.db_path, &fvk);
    let fvk = FVK(Mutex::new(fvk.clone()));
    db.create().unwrap();
    monitor_task(config.port).await;
    rocket.manage(db).manage(fvk)
        .mount(
            "/",
            routes![
                create_account,
                create_address,
                get_accounts,
                get_transaction,
                get_transfers,
                get_fee_estimate,
                get_height,
                sync_info,
                request_scan,
            ],
        )
        .attach(AdHoc::config::<WalletConfig>())
        .launch().await?;

    Ok(())
}

#[allow(dead_code)]
fn to_tonic<E: ToString>(e: E) -> tonic::Status {
    tonic::Status::internal(e.to_string())
}

fn from_tonic<E: ToString>(e: E) -> anyhow::Error {
    anyhow::anyhow!(e.to_string())
}

// TODO: Detect reorgs
