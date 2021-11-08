use bip39::{Seed, Mnemonic, Language};
use zcash_primitives::zip32::{ExtendedSpendingKey, ExtendedFullViewingKey, ChildIndex};
use zcash_client_backend::encoding::{encode_extended_spending_key, encode_extended_full_viewing_key, encode_payment_address};
use crate::NETWORK;
use zcash_primitives::consensus::Parameters;

pub struct Account {
    pub id: u32,
    pub esk: String,
    pub efvk: String,
    pub address: String,
}

pub fn derive_account(phrase: &str, account_index: u32) -> anyhow::Result<Account> {
    let mnemonic = Mnemonic::from_phrase(&phrase, Language::English)?;
    let seed = Seed::new(&mnemonic, "");
    let master = ExtendedSpendingKey::master(seed.as_bytes());
    let path = [
        ChildIndex::Hardened(32),
        ChildIndex::Hardened(NETWORK.coin_type()),
        ChildIndex::Hardened(account_index),
    ];
    let extsk = ExtendedSpendingKey::from_path(&master, &path);
    let esk = encode_extended_spending_key(NETWORK.hrp_sapling_extended_spending_key(), &extsk);
    let fvk = ExtendedFullViewingKey::from(&extsk);
    let (_, pa) = fvk.default_address().map_err(|_| anyhow::anyhow!("Invalid FVK"))?;
    let efvk =
        encode_extended_full_viewing_key(NETWORK.hrp_sapling_extended_full_viewing_key(), &fvk);
    let address = encode_payment_address(NETWORK.hrp_sapling_payment_address(), &pa);

    Ok(Account {
        id: account_index,
        esk,
        efvk,
        address,
    })
}

