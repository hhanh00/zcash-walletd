#[macro_use]
extern crate rocket;

#[path = "generated/cash.z.wallet.sdk.rpc.rs"]
pub mod lwd_rpc;

mod account;
mod db;
mod monitor;
mod network;
mod rpc;
mod scan;
mod transaction;

pub use crate::rpc::*;
use anyhow::{anyhow, Result};
use network::Network;
use std::str::FromStr;
use tonic::{transport::Channel, Request};
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt as _,
    util::SubscriberInitExt as _,
    EnvFilter, Layer, Registry,
};

pub type Hash = [u8; 32];
pub type Client = CompactTxStreamerClient<Channel>;

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

use crate::{
    db::Db,
    lwd_rpc::{compact_tx_streamer_client::CompactTxStreamerClient, BlockId},
    monitor::monitor_task,
    scan::ScanEvent,
};
use anyhow::Context;
use rocket::fairing::AdHoc;
use serde::Deserialize;
use zcash_client_backend::keys::UnifiedFullViewingKey;

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
        } else {
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
    let rocket = rocket::build();
    let figment = rocket.figment();
    let mut config: WalletConfig = figment.extract().unwrap();
    let ufvk = dotenv::var("VK")
        .context("Seed missing from .env file")
        .unwrap();
    let network = config.network();
    assert!(config.orchard);

    let notify_tx_url = dotenv::var("NOTIFY_TX_URL").ok();
    let ufvk = UnifiedFullViewingKey::decode(&network, &ufvk)
        .map_err(|_| anyhow!("Invalid Unified Viewing Key"))?;
    if let Some(notify_tx_url) = notify_tx_url {
        config.notify_tx_url = notify_tx_url;
    }
    let birth_height = dotenv::var("BIRTH_HEIGHT")
        .ok()
        .map(|h| u32::from_str(&h).unwrap())
        .expect("Birth Height MUST be specified");
    let db = Db::new(network, &config.db_path, &ufvk).await?;
    let db_exists = db.create().await?;
    if !db_exists {
        db.new_account("").await?;
        let mut client = CompactTxStreamerClient::connect(config.lwd_url.clone()).await?;
        let b = client
            .get_block(Request::new(BlockId {
                height: birth_height as u64,
                hash: vec![],
            }))
            .await?
            .into_inner();
        let hash: Hash = b.hash.try_into().unwrap();
        db.store_events(&[ScanEvent::Block(birth_height, hash)])
            .await?;
    }

    // monitor_task(config.port, config.poll_interval).await;
    rocket
        .manage(db)
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
        .launch()
        .await?;

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
