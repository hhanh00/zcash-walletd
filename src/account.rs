use rocket::serde::{Deserialize, Serialize};

pub struct Account {
    pub account_index: u32,
    pub address: String,
}

pub struct SubAccount {
    pub account_index: u32,
    pub sub_account_index: u32,
    pub address: String,
}

#[derive(Serialize, Deserialize)]
pub struct AccountBalance {
    pub account_index: u32,
    pub balance: u64,
    pub base_address: String,
    pub label: String,
    pub tag: String,
    pub unlocked_balance: u64,
}
