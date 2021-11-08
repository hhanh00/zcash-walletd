use rocket::serde::{Serialize, Deserialize, json::Json};

#[derive(Serialize, Deserialize)]
pub struct CreateAccountRequest {
}

#[derive(Serialize, Deserialize)]
pub struct CreateAccountResponse {
}

#[post("/create_account", data = "<request>")]
pub fn create_account(request: Json<CreateAccountRequest>) -> Json<CreateAccountResponse> {
    let rep = CreateAccountResponse {
    };
    Json(rep)
}
#[derive(Serialize, Deserialize)]
pub struct CreateAddressRequest {
}

#[derive(Serialize, Deserialize)]
pub struct CreateAddressResponse {
}

#[post("/create_address", data = "<request>")]
pub fn create_address(request: Json<CreateAddressRequest>) -> Json<CreateAddressResponse> {
    let rep = CreateAddressResponse {
    };
    Json(rep)
}
#[derive(Serialize, Deserialize)]
pub struct GetAccountRequest {
}

#[derive(Serialize, Deserialize)]
pub struct GetAccountResponse {
}

#[post("/get_account", data = "<request>")]
pub fn get_account(request: Json<GetAccountRequest>) -> Json<GetAccountResponse> {
    let rep = GetAccountResponse {
    };
    Json(rep)
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
