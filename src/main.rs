#[macro_use]
extern crate rocket;

#[path = "generated/cash.z.wallet.sdk.rpc.rs"]
pub mod lwd_rpc;

mod account;
mod db;
mod rpc;
mod scan;
mod transaction;

use std::str::FromStr;
pub use crate::rpc::*;
use sapling_crypto::zip32::ExtendedFullViewingKey;
use zcash_primitives::consensus::{Network, NetworkConstants as _};

use clap::Parser;

#[derive(Parser, Debug)]
#[clap(about, version, author)]
struct Args {
    #[clap(short, long)]
    rescan: bool,
}

// pub const NETWORK: Network = Network::TestNetwork;
pub const NETWORK: Network = Network::MainNetwork;

// They come from the config file
//
// const DB_PATH: &str = "zec-wallet.db";
// const CONFIRMATIONS: u32 = 10;
//
// pub const LWD_URL: &str = "https://lite.ycash.xyz:9067";
// pub const NOTIFY_TX_URL: &str = "https://localhost:14142/zcashlikedaemoncallback/tx?cryptoCode=yec&hash=";

use crate::db::Db;
use anyhow::Context;
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
    poll_interval: u16,
}

#[rocket::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();
    env_logger::init();
    let args: Args = Args::parse();
    let fvk = dotenv::var("VK")
        .context("Seed missing from .env file")
        .unwrap();
    let lwd_url = dotenv::var("LWD_URL").ok();
    let notify_tx_url = dotenv::var("NOTIFY_TX_URL").ok();
    let confirmations = dotenv::var("CONFIRMATIONS").ok().map(|c| u32::from_str(&c).unwrap());
    let fvk = decode_extended_full_viewing_key(NETWORK.hrp_sapling_extended_full_viewing_key(), &fvk).expect("Invalid viewing key");
    let rocket = rocket::build();
    let figment = rocket.figment();
    let mut config: WalletConfig = figment.extract().unwrap();
    if let Some(notify_tx_url) = notify_tx_url {
        config.notify_tx_url = notify_tx_url;
    }
    if let Some(lwd_url) = lwd_url {
        config.lwd_url = lwd_url;
    }
    if let Some(confirmations) = confirmations {
        config.confirmations = confirmations;
    }
    let db = Db::new(&config.db_path, &fvk);
    let fvk = FVK(Mutex::new(fvk.clone()));
    let db_exists = db.create().unwrap();
    if !db_exists {
        db.new_account("")?;
    }
    let birth_height =
        if !db_exists || args.rescan {
            dotenv::var("BIRTH_HEIGHT").ok().map(|h| u32::from_str(&h).unwrap())
        }
    else { None };

    monitor_task(birth_height, config.port, config.poll_interval).await;
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
