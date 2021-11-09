use rocket::serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize)]
pub struct SubAddress {
    pub major: u32,
    pub minor: u32,
}

#[derive(Default, Serialize, Deserialize)]
pub struct Transfer {
    pub address: String,
    pub amount: u64,
    pub confirmations: u32,
    pub height: u32,
    pub fee: u64,
    pub note: String,
    pub payment_id: String,
    pub subaddr_index: SubAddress,
    pub suggested_confirmations_threshold: u32,
    pub timestamp: u64,
    pub txid: String,
    pub r#type: String,
    pub unlock_time: u32,
}
