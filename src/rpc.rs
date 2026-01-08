use crate::account::AccountBalance;
use crate::db::Db;
use crate::lwd_rpc::compact_tx_streamer_client::CompactTxStreamerClient;
use crate::lwd_rpc::*;
use crate::scan::{get_latest_height, Decoder, Orchard, Sapling, ScanError};
use crate::transaction::Transfer;
use crate::{from_tonic, WalletConfig};
use anyhow::{anyhow, Result};
use rocket::response::Debug;
use rocket::serde::{json::Json, Deserialize, Serialize};
use rocket::State;
use std::time::Instant;
use tokio::time::{sleep, Duration};
use tonic::Request;

#[derive(Serialize, Deserialize)]
pub struct CreateAccountRequest {
    label: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct CreateAccountResponse {
    account_index: u32,
    address: String,
}

#[post("/create_account", data = "<request>")]
pub async fn create_account(
    request: Json<CreateAccountRequest>,
    db: &State<Db>,
) -> Result<Json<CreateAccountResponse>, Debug<anyhow::Error>> {
    let request = request.into_inner();
    let name = request.label.unwrap_or("".to_string());

    let account = db.new_account(&name).await?;
    let rep = CreateAccountResponse {
        account_index: account.account_index,
        address: account.address,
    };

    Ok(Json(rep))
}
#[derive(Serialize, Deserialize)]
pub struct CreateAddressRequest {
    account_index: u32,
    label: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct CreateAddressResponse {
    address: String,
    address_index: u32,
}

#[post("/create_address", data = "<request>")]
pub async fn create_address(
    request: Json<CreateAddressRequest>,
    db: &State<Db>,
) -> Result<Json<CreateAddressResponse>, Debug<anyhow::Error>> {
    let request = request.into_inner();
    let name = request.label.unwrap_or("".to_string());
    let sub_account = db.new_sub_account(request.account_index, &name).await?;

    let rep = CreateAddressResponse {
        address: sub_account.address.clone(),
        address_index: sub_account.sub_account_index,
    };
    Ok(Json(rep))
}
#[derive(Serialize, Deserialize)]
pub struct GetAccountsRequest {
    tag: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct GetAccountsResponse {
    subaddress_accounts: Vec<AccountBalance>,
    total_balance: u64,
    total_unlocked_balance: u64,
}

#[post("/get_accounts", data = "<_request>")]
pub async fn get_accounts(
    _request: Json<GetAccountsRequest>,
    db: &State<Db>,
    config: &State<WalletConfig>,
) -> Result<Json<GetAccountsResponse>, Debug<anyhow::Error>> {
    let mut client = CompactTxStreamerClient::connect(config.lwd_url.clone())
        .await
        .map_err(from_tonic)?;
    let latest_height = get_latest_height(&mut client).await?;
    let sub_accounts = db.get_accounts(latest_height, config.confirmations).await?;
    let total_balance: u64 = sub_accounts.iter().map(|sa| sa.balance).sum();
    let total_unlocked_balance: u64 = sub_accounts.iter().map(|sa| sa.unlocked_balance).sum();

    let rep = GetAccountsResponse {
        subaddress_accounts: sub_accounts,
        total_balance,
        total_unlocked_balance,
    };
    Ok(Json(rep))
}

#[derive(Serialize, Deserialize)]
pub struct GetTransactionByIdRequest {
    pub txid: String,
    pub account_index: u32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GetTransactionByIdResponse {
    pub transfer: Transfer,
    pub transfers: Vec<Transfer>,
}

#[post("/get_transfer_by_txid", data = "<request>")]
pub async fn get_transaction(
    request: Json<GetTransactionByIdRequest>,
    db: &State<Db>,
    config: &State<WalletConfig>,
) -> Result<Json<GetTransactionByIdResponse>, Debug<anyhow::Error>> {
    let request = request.into_inner();
    let mut client = CompactTxStreamerClient::connect(config.lwd_url.clone())
        .await
        .map_err(from_tonic)?;
    let latest_height = get_latest_height(&mut client).await?;
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut backoff_ms: u64 = 100;

    let transfers = loop {
        let transfers = db
            .get_transfers_by_txid(
                latest_height,
                &request.txid,
                request.account_index,
                config.confirmations,
            )
            .await?;

        if !transfers.is_empty() {
            break transfers;
        }

        if Instant::now() >= deadline {
            break transfers;
        }

        sleep(Duration::from_millis(backoff_ms)).await;
        backoff_ms = (backoff_ms * 2).min(1000);
    };

    if transfers.is_empty() {
        return Err(Debug(anyhow!(
            "transfer not yet available for txid {} (account_index {})",
            request.txid,
            request.account_index
        )));
    }
    let rep = GetTransactionByIdResponse {
        transfer: transfers[0].clone(),
        transfers,
    };
    info!("{rep:?}");
    Ok(Json(rep))
}

#[derive(Serialize, Deserialize)]
pub struct GetTransfersRequest {
    pub account_index: u32,
    pub r#in: bool,
    pub subaddr_indices: Vec<u32>,
}

#[derive(Serialize, Deserialize)]
pub struct GetTransfersResponse {
    pub r#in: Vec<Transfer>,
}

#[post("/get_transfers", data = "<request>")]
pub async fn get_transfers(
    request: Json<GetTransfersRequest>,
    db: &State<Db>,
    config: &State<WalletConfig>,
) -> Result<Json<GetTransfersResponse>, Debug<anyhow::Error>> {
    let request = request.into_inner();
    assert!(request.r#in);
    let mut client = CompactTxStreamerClient::connect(config.lwd_url.clone())
        .await
        .map_err(from_tonic)?;
    let latest_height = get_latest_height(&mut client).await?;
    let transfers = db
        .get_transfers(
            latest_height,
            request.account_index,
            &request.subaddr_indices,
            config.confirmations,
        )
        .await?;
    let rep = GetTransfersResponse { r#in: transfers };
    Ok(Json(rep))
}

#[derive(Serialize, Deserialize)]
pub struct GetFeeEstimateRequest {}

#[derive(Serialize, Deserialize)]
pub struct GetFeeEstimateResponse {
    pub fee: u64,
}

// Roughly estimate at 2 transparent in/out + 2 shielded in/out
// We cannot implement ZIP-321 here because we don't have
// the transaction
const LOGICAL_ACTION_FEE: u64 = 5000u64;

#[post("/get_fee_estimate", data = "<_request>")]
pub fn get_fee_estimate(
    _request: Json<GetFeeEstimateRequest>,
) -> Result<Json<GetFeeEstimateResponse>, Debug<anyhow::Error>> {
    let rep = GetFeeEstimateResponse {
        fee: 4 * LOGICAL_ACTION_FEE,
    };
    Ok(Json(rep))
}

#[derive(Serialize, Deserialize)]
pub struct GetHeightRequest {}

#[derive(Serialize, Deserialize)]
pub struct GetHeightResponse {
    pub height: u32,
}

#[post("/get_height", data = "<_request>")]
pub async fn get_height(
    _request: Json<GetHeightRequest>,
    config: &State<WalletConfig>,
) -> Result<Json<GetHeightResponse>, Debug<anyhow::Error>> {
    let mut client = CompactTxStreamerClient::connect(config.lwd_url.clone())
        .await
        .map_err(from_tonic)?;
    let latest_height = get_latest_height(&mut client).await?;
    let rep = GetHeightResponse {
        height: latest_height,
    };
    Ok(Json(rep))
}

#[derive(Serialize, Deserialize)]
pub struct SyncInfoRequest {}

#[derive(Serialize, Deserialize)]
pub struct SyncInfoResponse {
    pub target_height: u32,
    pub height: u32,
}

#[post("/sync_info", data = "<_request>")]
pub async fn sync_info(
    _request: Json<SyncInfoRequest>,
    config: &State<WalletConfig>,
) -> Result<Json<SyncInfoResponse>, Debug<anyhow::Error>> {
    let mut client = CompactTxStreamerClient::connect(config.lwd_url.clone())
        .await
        .map_err(from_tonic)?;
    let rep = client
        .get_lightd_info(Request::new(Empty {}))
        .await
        .map_err(from_tonic)?
        .into_inner();
    let rep = SyncInfoResponse {
        target_height: rep.block_height as u32,
        height: rep.estimated_height as u32,
    };
    Ok(Json(rep))
}

#[post("/request_scan")]
pub async fn request_scan(
    db: &State<Db>,
    config: &State<WalletConfig>,
) -> Result<(), Debug<anyhow::Error>> {
    let network = config.network();
    let ufvk = db.ufvk();
    let start = db.get_synced_height().await?;
    let prev_hash = db
        .get_block_hash(start)
        .await?
        .ok_or(anyhow::anyhow!("Block Hash missing from db"))?;

    let nfs = db.get_nfs().await?;
    let mut sap_dec = ufvk.sapling().map(|fvk| {
        let nk = fvk.fvk().vk.nk;
        let ivk = fvk.to_ivk(zip32::Scope::External);
        let pivk = sapling_crypto::keys::PreparedIncomingViewingKey::new(&ivk);
        // TODO: Load nfs
        Decoder::<Sapling>::new(nk, fvk.clone(), pivk, &nfs)
    });
    let mut orc_dec = ufvk.orchard().map(|fvk| {
        let ivk = fvk.to_ivk(zip32::Scope::External);
        let pivk = orchard::keys::PreparedIncomingViewingKey::new(&ivk);
        // TODO: Load nfs
        Decoder::<Orchard>::new(fvk.clone(), ivk, pivk, &nfs)
    });

    let mut client = CompactTxStreamerClient::connect(config.lwd_url.clone())
        .await
        .map_err(anyhow::Error::new)?;
    let end = get_latest_height(&mut client).await?;

    info!("Scan from {start} to {end}");
    if start >= end {
        return Ok(());
    }

    let res = crate::scan::scan(
        &network,
        &mut client,
        start + 1,
        end,
        &prev_hash,
        &mut sap_dec,
        &mut orc_dec,
    )
    .await;
    match res {
        Err(error) =>
        // Rewind if we hit a chain reorg but don't error
        {
            match error {
                ScanError::Reorganization => {
                    let synced_height = db.get_synced_height().await?;
                    db.truncate_height(synced_height - config.confirmations)
                        .await
                }
                ScanError::Other(error) => Err(error),
            }?
        }

        Ok(events) => {
            db.store_events(&events).await?;
        }
    }
    Ok(())
}

pub async fn notify_tx(txid: &[u8], notify_tx_url: &str) -> Result<()> {
    let mut txid = txid.to_vec();
    txid.reverse();
    let txid = hex::encode(&txid);
    info!("Notify tx {}", &txid);

    let url = notify_tx_url.to_string() + &txid;
    // TODO: Remove self signed certificate accept
    let res = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()?
        .get(url)
        .send()
        .await;
    if let Err(e) = res {
        log::warn!("Failed to notify new tx: {e}",);
    }

    Ok(())
}
