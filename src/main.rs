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
pub mod scan2;

use anyhow::{anyhow, Result};
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{fmt::{self, format::FmtSpan}, layer::SubscriberExt as _, util::SubscriberInitExt as _, EnvFilter, Layer, Registry};
use std::str::FromStr;
pub use crate::rpc::*;
use network::Network;

use clap::Parser;

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
use anyhow::Context;
use zcash_client_backend::keys::UnifiedFullViewingKey;
use crate::scan::monitor_task;
use rocket::fairing::AdHoc;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct WalletConfig {
    port: u16,
    db_path: String,
    confirmations: u32,
    lwd_url: String,
    notify_tx_url: String,
    poll_interval: u16,
    regtest: bool,
    orchard: bool,
}

impl WalletConfig {
    pub fn network(&self) -> Network {
        if self.regtest {
            Network::Regtest
        }
        else {
            Network::Main
        }
    }
}

#[rocket::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();
    env_logger::init();
    let _ = Registry::default()
        .with(default_layer())
        .with(env_layer())
        .try_init();
    let args: Args = Args::parse();
    let rocket = rocket::build();
    let figment = rocket.figment();
    let mut config: WalletConfig = figment.extract().unwrap();
    let ufvk = dotenv::var("VK")
        .context("Seed missing from .env file")
        .unwrap();
    let network = config.network();
    assert!(config.orchard);

    let notify_tx_url = dotenv::var("NOTIFY_TX_URL").ok();
    let ufvk = UnifiedFullViewingKey::decode(&network, &ufvk).map_err(|_| anyhow!("Invalid Unified Viewing Key"))?;
    if let Some(notify_tx_url) = notify_tx_url {
        config.notify_tx_url = notify_tx_url;
    }
    let db = Db::new(network, &config.db_path, &ufvk).await?;
    let db_exists = db.create().await?;
    if !db_exists {
        db.new_account("").await?;
    }
    let birth_height =
        if !db_exists || args.rescan {
            dotenv::var("BIRTH_HEIGHT").ok().map(|h| u32::from_str(&h).unwrap())
        }
    else { None };

    monitor_task(birth_height, config.port, config.poll_interval).await;
    rocket.manage(db)
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

type BoxedLayer<S> = Box<dyn Layer<S> + Send + Sync + 'static>;

fn default_layer<S>() -> BoxedLayer<S>
where
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    fmt::layer()
        .with_ansi(false)
        .with_span_events(FmtSpan::ACTIVE)
        .compact()
        .boxed()
}

fn env_layer<S>() -> BoxedLayer<S>
where
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy()
        .boxed()
}
