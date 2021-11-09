use crate::account::AccountBalance;
use crate::db::Db;
use crate::transaction::Transfer;
use rocket::response::Debug;
use rocket::serde::{json::Json, Deserialize, Serialize};
use rocket::State;
use crate::scan::{scan_blocks, scan_transaction, get_latest_height};
use crate::{LWD_URL, FVK, from_tonic};
use tokio_stream::StreamExt;
use crate::lwd_rpc::compact_tx_streamer_client::CompactTxStreamerClient;
use zcash_primitives::zip32::ExtendedFullViewingKey;
use tonic::Request;
use crate::lwd_rpc::ChainSpec;

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
pub fn create_account(
    request: Json<CreateAccountRequest>,
    db: &State<Db>,
) -> Result<Json<CreateAccountResponse>, Debug<anyhow::Error>> {
    let request = request.into_inner();
    let name = request.label.unwrap_or("".to_string());

    let account = db.new_account(&name)?;
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
pub fn create_address(
    request: Json<CreateAddressRequest>,
    db: &State<Db>,
) -> Result<Json<CreateAddressResponse>, Debug<anyhow::Error>> {
    let request = request.into_inner();
    let name = request.label.unwrap_or("".to_string());
    let sub_account = db.new_sub_account(request.account_index, &name)?;

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
) -> Result<Json<GetAccountsResponse>, Debug<anyhow::Error>> {
    let mut client = CompactTxStreamerClient::connect(LWD_URL.to_string()).await.map_err(from_tonic)?;
    let latest_height = get_latest_height(&mut client).await?;
    let sub_accounts = db.get_accounts(latest_height)?;
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

#[derive(Serialize, Deserialize)]
pub struct GetTransactionByIdResponse {
    pub transfers: Vec<Transfer>,
}

#[post("/get_transfer_by_txid", data = "<request>")]
pub async fn get_transaction(
    request: Json<GetTransactionByIdRequest>,
    db: &State<Db>,
) -> Result<Json<GetTransactionByIdResponse>, Debug<anyhow::Error>> {
    let request = request.into_inner();
    let mut client = CompactTxStreamerClient::connect(LWD_URL.to_string()).await.map_err(from_tonic)?;
    let latest_height = get_latest_height(&mut client).await?;
    let transfers = db.get_transfers_by_txid(latest_height, &request.txid, request.account_index)?;
    let rep = GetTransactionByIdResponse {
        transfers,
    };
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
) -> Result<Json<GetTransfersResponse>, Debug<anyhow::Error>> {
    let request = request.into_inner();
    assert!(request.r#in);
    let mut client = CompactTxStreamerClient::connect(LWD_URL.to_string()).await.map_err(from_tonic)?;
    let latest_height = get_latest_height(&mut client).await?;
    let transfers = db.get_transfers(latest_height, request.account_index, &request.subaddr_indices)?;
    let rep = GetTransfersResponse {
        r#in: transfers,
    };
    Ok(Json(rep))
}

#[derive(Serialize, Deserialize)]
pub struct MakePaymentRequest {}

#[derive(Serialize, Deserialize)]
pub struct MakePaymentResponse {}

#[post("/make_payment", data = "<request>")]
pub fn make_payment(
    request: Json<MakePaymentRequest>,
) -> Result<Json<MakePaymentResponse>, Debug<anyhow::Error>> {
    let rep = MakePaymentResponse {};
    Ok(Json(rep))
}

#[derive(Serialize, Deserialize)]
pub struct ScanRequest {
    start_height: u32,
}

#[derive(Serialize, Deserialize)]
pub struct ScanResponse {}

#[post("/request_scan", data = "<request>")]
pub async fn request_scan(
    request: Json<ScanRequest>,
    db: &State<Db>,
    fvk: &State<FVK>,
) -> Result<Json<ScanResponse>, Debug<anyhow::Error>> {
    let request = request.into_inner();
    let fvk: ExtendedFullViewingKey = fvk.0.lock().unwrap().clone();
    let vk = fvk.fvk.vk.clone();
    let ivk = vk.ivk();

    let mut client = CompactTxStreamerClient::connect(LWD_URL.to_string()).await.map_err(from_tonic)?;
    let mut stream = scan_blocks(request.start_height, LWD_URL, &fvk).await?;
    let mut nf_map = db.get_nfs()?;
    while let Some(tx_index) = stream.next().await {
        let (spends, outputs, value) = scan_transaction(&mut client, tx_index.height, tx_index.tx_id, tx_index.position, &vk, &ivk, &nf_map).await?;
        let id_tx = db.store_tx(&tx_index.tx_id.0, tx_index.height, value)?;
        for id_note in spends.iter() {
            db.mark_spent(*id_note, id_tx)?;
        }
        for n in outputs.iter() {
            println!("{}", hex::encode(&n.nf));
            let id_note = db.store_note(n, id_tx)?;
            nf_map.insert(n.nf, id_note);
        }
        // println!("{} {}", hex::encode(tx_index.tx_id.0), tx_index.position);
    }
    let rep = ScanResponse {};
    Ok(Json(rep))
}
