use rocket::serde::{Serialize, Deserialize, json::Json};
use crate::account::derive_account;
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
pub struct GetAccountRequest {
}

#[derive(Serialize, Deserialize)]
pub struct GetAccountResponse {
    account_index: u32,
    address: String,
}

#[post("/get_account", data = "<_request>")]
pub fn get_account(_request: Json<GetAccountRequest>) -> Result<Json<GetAccountResponse>, Debug<anyhow::Error>> {
    let seed = dotenv::var("SEED").context("Seed missing from .env file")?;
    let account_index = 0; // TODO

    let keys = derive_account(&seed, account_index)?;
    let rep = GetAccountResponse {
        account_index,
        address: keys.address,
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
