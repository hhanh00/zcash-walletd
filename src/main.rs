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
use figment::providers::{Env, Format, Json};
use network::Network;
use std::path::Path;
use tonic::transport::Channel;
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
    db::Db, lwd_rpc::compact_tx_streamer_client::CompactTxStreamerClient, monitor::monitor_task,
};
use serde::Deserialize;
use zcash_client_backend::keys::UnifiedFullViewingKey;

#[derive(Deserialize, Debug)]
pub struct WalletConfig {
    port: u16,
    db_path: String,
    confirmations: u32,
    lwd_url: String,
    notify_tx_url: String,
    poll_interval: u16,
    regtest: bool,
    orchard: bool,
    vk: String,
    birth_height: u32,
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
    let config_path = dotenv::var("CONFIG_PATH")
        .ok()
        .unwrap_or("/data/config.json".to_string());
    let _ = Registry::default()
        .with(default_layer())
        .with(env_layer())
        .try_init();
    let rocket = rocket::build();
    let mut figment = rocket.figment().clone();
    figment = figment.merge(Env::raw());
    info!("figment {figment:?}");
    let config = Path::new(&config_path);
    if config.exists() {
        figment = figment.merge(Json::file(config_path));
    }

    let config: WalletConfig = figment.extract().unwrap();
    info!("Config {config:?}");
    let network = config.network();
    assert!(config.orchard);

    let ufvk = &config.vk;
    let birth_height = config.birth_height;
    let ufvk = UnifiedFullViewingKey::decode(&network, ufvk)
        .map_err(|_| anyhow!("Invalid Unified Viewing Key"))?;
    let db = Db::new(network, &config.db_path, &ufvk, &config.notify_tx_url).await?;
    let db_exists = db.create().await?;
    if !db_exists {
        db.new_account("").await?;
    }
    let mut client = CompactTxStreamerClient::connect(config.lwd_url.clone()).await?;
    db.fetch_block_hash(&mut client, birth_height).await?;

    monitor_task(config.port, config.poll_interval).await;
    rocket
        .manage(db)
        .manage(config)
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
                reorg,
            ],
        )
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
