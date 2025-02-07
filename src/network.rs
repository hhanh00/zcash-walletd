use zcash_protocol::{
    consensus::{BlockHeight, MainNetwork, NetworkUpgrade, Parameters},
    local_consensus::LocalNetwork,
};

#[derive(Copy, Clone, Debug)]
pub enum Network {
    Main,
    Regtest,
}

impl Parameters for Network {
    fn network_type(&self) -> zcash_protocol::consensus::NetworkType {
        match self {
            Network::Main => MainNetwork.network_type(),
            Network::Regtest => REGTEST.network_type(),
        }
    }

    fn activation_height(
        &self,
        nu: NetworkUpgrade,
    ) -> Option<zcash_protocol::consensus::BlockHeight> {
        match self {
            Network::Main => MainNetwork.activation_height(nu),
            Network::Regtest => REGTEST.activation_height(nu),
        }
    }
}

pub const REGTEST: LocalNetwork = LocalNetwork {
    overwinter: Some(BlockHeight::from_u32(1)),
    sapling: Some(BlockHeight::from_u32(1)),
    blossom: Some(BlockHeight::from_u32(1)),
    heartwood: Some(BlockHeight::from_u32(1)),
    canopy: Some(BlockHeight::from_u32(1)),
    nu5: Some(BlockHeight::from_u32(1)),
    nu6: Some(BlockHeight::from_u32(1)),
};
