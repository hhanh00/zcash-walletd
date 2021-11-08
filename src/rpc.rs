use rocket::serde::{Serialize, Deserialize, json::Json};
use crate::account::AccountBalance;
use rocket::response::Debug;
use rocket::State;
use crate::db::Db;
use crate::transaction::Transfer;

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
pub fn create_account(request: Json<CreateAccountRequest>, db: &State<Db>) -> Result<Json<CreateAccountResponse>, Debug<anyhow::Error>> {
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
pub fn create_address(request: Json<CreateAddressRequest>, db: &State<Db>) -> Result<Json<CreateAddressResponse>, Debug<anyhow::Error>> {
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

#[post("/get_accounts", data = "<request>")]
pub fn get_accounts(request: Json<GetAccountsRequest>, db: &State<Db>) -> Result<Json<GetAccountsResponse>, Debug<anyhow::Error>> {
    let request = request.into_inner();

    let sub_accounts = db.get_accounts()?;
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
    pub transfer: Transfer,
    pub transfers: Vec<Transfer>,
}

#[post("/get_transaction", data = "<request>")]
pub fn get_transaction(request: Json<GetTransactionByIdRequest>) -> Json<GetTransactionByIdResponse> {
    let rep = GetTransactionByIdResponse {
        transfer: Transfer::default(),
        transfers: vec![],
    };
    Json(rep)
}
#[derive(Serialize, Deserialize)]
pub struct MakePaymentRequest {
}

#[derive(Serialize, Deserialize)]
pub struct MakePaymentResponse {
}

#[post("/make_payment", data = "<request>")]
pub fn make_payment(request: Json<MakePaymentRequest>) -> Json<MakePaymentResponse> {
    let rep = MakePaymentResponse {
    };
    Json(rep)
}
