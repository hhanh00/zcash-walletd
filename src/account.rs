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

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use crate::account::Account;
    use bip39::{Language, Mnemonic, Seed};
    use zcash_client_backend::encoding::{
        encode_extended_full_viewing_key, encode_extended_spending_key, encode_payment_address,
    };
    use zcash_primitives::{consensus::NetworkConstants as _, zip32::ChildIndex};
    use sapling_crypto::zip32::ExtendedSpendingKey;

    #[allow(dead_code)]
    fn derive_account(phrase: &str, account_index: u32) -> Result<Account> {
        let network = crate::network::Network::Regtest;
        let mnemonic = Mnemonic::from_phrase(phrase, Language::English)?;
        let seed = Seed::new(&mnemonic, "");
        let master = ExtendedSpendingKey::master(seed.as_bytes());
        let path = [
            ChildIndex::hardened(32),
            ChildIndex::hardened(network.coin_type()),
            ChildIndex::hardened(account_index),
        ];
        let extsk = ExtendedSpendingKey::from_path(&master, &path);
        let esk = encode_extended_spending_key(network.hrp_sapling_extended_spending_key(), &extsk);
        #[allow(deprecated)]
        let fvk = extsk.to_extended_full_viewing_key();
        let (_, pa) = fvk
            .default_address();
        let efvk =
            encode_extended_full_viewing_key(network.hrp_sapling_extended_full_viewing_key(), &fvk);
        let address = encode_payment_address(network.hrp_sapling_payment_address(), &pa);

        tracing::info!("{} {}", esk, efvk);

        Ok(Account {
            account_index,
            address,
        })
    }

    #[test]
    fn test_seed() {
        dotenv::dotenv().unwrap();
        let seed = dotenv::var("SEED").unwrap();
        let account = derive_account(&seed, 0).unwrap();
        println!("{}", account.address);
    }
}
