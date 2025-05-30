#[macro_use]
extern crate rocket;

#[path = "generated/cash.z.wallet.sdk.rpc.rs"]
pub mod lwd_rpc;

mod network;
mod account;
mod db;
mod rpc;
mod scan;
mod transaction;

use std::str::FromStr;
pub use crate::rpc::*;
use network::Network;
use sapling_crypto::zip32::ExtendedFullViewingKey;

use clap::Parser;
use zcash_protocol::consensus::NetworkConstants as _;

#[derive(Parser, Debug)]
#[clap(about, version, author)]
struct Args {
    #[clap(short, long)]
    rescan: bool,
}

// They come from the config file
//
// const DB_PATH: &str = "zec-wallet.db";
// const CONFIRMATIONS: u32 = 10;
//
// pub const LWD_URL: &str = "https://lite.ycash.xyz:9067";
// pub const NOTIFY_TX_URL: &str = "https://localhost:14142/zcashlikedaemoncallback/tx?cryptoCode=yec&hash=";

use crate::db::Db;
use std::sync::Mutex;
use zcash_client_backend::encoding::decode_extended_full_viewing_key;
use crate::scan::monitor_task;
use rocket::fairing::AdHoc;
use serde::{Deserialize, Serialize};
use figment::{Figment, providers::{Json, Format, Env}};
use std::path::Path;

pub struct FVK(pub Mutex<ExtendedFullViewingKey>);

#[derive(Debug, Deserialize, Serialize)]
pub struct WalletConfig {
    port: u16,
    db_path: String,
    config_path: Option<String>,
    vk: Option<String>,
    confirmations: u32,
    lwd_url: String,
    notify_tx_url: String,
    poll_interval: u16,
    regtest: bool,
}

impl WalletConfig {
    pub fn network(&self) -> Network {
        let network = if self.regtest {
            Network::Regtest
        }
        else {
            Network::Main
        };
        network
    }
}

#[derive(Debug, Deserialize)]
struct DataConfig {
    confirmations: Option<u32>,
    birth_height: Option<u32>
}

#[rocket::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();
    env_logger::init();
    let args: Args = Args::parse();
    let data_config: DataConfig = Figment::new()
        .merge(Json::file(Path::new("/data/config.json")))
        .extract()?;

    let mut figment = rocket::Config::figment()
        .merge(Json::file("/data/config.json"))
        .merge(Env::raw()
            .only(&["NOTIFY_TX_URL", "LWD_URL", "VK", "CONFIRMATIONS"])
            .global()
        );
    if let Some(confirmations) = data_config.confirmations {
        figment = figment.merge(("confirmations", confirmations));
    }
    let config: WalletConfig = figment.extract().unwrap();

    let fvk = dotenv::var("VK")
        .ok()
        .or(config.vk.clone())
        .expect("VK missing from .env file or data config");
    let network = config.network();

    let fvk = decode_extended_full_viewing_key(network.hrp_sapling_extended_full_viewing_key(), &fvk).expect("Invalid viewing key");
    let db = Db::new(network, &config.db_path, &fvk);
    let fvk = FVK(Mutex::new(fvk.clone()));
    let db_exists = db.create().unwrap();
    if !db_exists {
        db.new_account("")?;
    }
    let birth_height =
        if !db_exists || args.rescan {
            dotenv::var("BIRTH_HEIGHT")
            .ok()
            .and_then(|h| u32::from_str(&h).ok())
            .or(data_config.birth_height.clone())
            .or_else(|| {
                panic!("BIRTH_HEIGHT missing from .env file or data config");
            })
        }
    else { None };

    monitor_task(birth_height, config.port, config.poll_interval).await;

    rocket::custom(figment)
        .manage(db)
        .manage(fvk)
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
