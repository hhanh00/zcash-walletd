use rocket::serde::{Serialize, Deserialize, json::Json};
use crate::account::{derive_account, SubAccount};
use rocket::response::Debug;
use anyhow::Context;
use rocket::State;
use crate::db::Db;

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
    let seed = dotenv::var("SEED").context("Seed missing from .env file")?;
    let name = request.label.unwrap_or("".to_string());

    let account = db.derive_account(&name, &seed)?;
    let rep = CreateAccountResponse {
        account_index: account.id,
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
    address_index: i64,
}

#[post("/create_address", data = "<request>")]
pub fn create_address(request: Json<CreateAddressRequest>, db: &State<Db>) -> Result<Json<CreateAddressResponse>, Debug<anyhow::Error>> {
    let request = request.into_inner();
    let name = request.label.unwrap_or("".to_string());
    let diversified_address = db.new_diversified_address(request.account_index, &name)?;

    let rep = CreateAddressResponse {
        address: diversified_address.address.clone(),
        address_index: diversified_address.index,
    };
    Ok(Json(rep))
}
#[derive(Serialize, Deserialize)]
pub struct GetAccountsRequest {
    tag: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct GetAccountsResponse {
    subaddress_accounts: Vec<SubAccount>,
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
}

#[derive(Serialize, Deserialize)]
pub struct GetTransactionByIdResponse {
}

#[post("/get_transaction", data = "<request>")]
pub fn get_transaction(request: Json<GetTransactionByIdRequest>) -> Json<GetTransactionByIdResponse> {
    let rep = GetTransactionByIdResponse {
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
