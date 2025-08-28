use anyhow::Result;
use orchard::{
    note::{ExtractedNoteCommitment, Nullifier},
    note_encryption::{CompactAction, OrchardDomain},
};
use sapling_crypto::note_encryption::{SaplingDomain, Zip212Enforcement};
use tonic::Request;
use zcash_keys::keys::UnifiedFullViewingKey;
use zcash_note_encryption::{try_compact_note_decryption, EphemeralKeyBytes, ShieldedOutput};
use zcash_primitives::merkle_tree::{read_commitment_tree, HashSer};

use crate::{
    lwd_rpc::{
        compact_tx_streamer_client::CompactTxStreamerClient, BlockId, BlockRange,
        CompactOrchardAction, CompactSaplingOutput,
    },
    network::Network,
};

pub type Hash = [u8; 32];

pub async fn scan(
    _network: &Network,
    lwd_url: &str,
    start: u32,
    end: u32,
    prev_hash: &Hash,
    ufvk: &UnifiedFullViewingKey,
) -> Result<Vec<WalletTx>> {
    let mut client = CompactTxStreamerClient::connect(lwd_url.to_string()).await?;

    let tree_state = client
        .get_tree_state(Request::new(BlockId {
            height: start as u64,
            hash: vec![],
        }))
        .await?
        .into_inner();
    let sap_dec = ufvk.sapling().map(|fvk| {
        let ivk = fvk.to_ivk(zcash_primitives::zip32::Scope::External);
        let pivk = sapling_crypto::keys::PreparedIncomingViewingKey::new(&ivk);
        Decoder::<Sapling>::new(pivk)
    });
    let orc_dec = ufvk.orchard().map(|fvk| {
        let ivk = fvk.to_ivk(zcash_primitives::zip32::Scope::External);
        let pivk = orchard::keys::PreparedIncomingViewingKey::new(&ivk);
        Decoder::<Orchard>::new(pivk)
    });

    let mut blocks = client
        .get_block_range(Request::new(BlockRange {
            start: Some(BlockId {
                height: start as u64,
                hash: vec![],
            }),
            end: Some(BlockId {
                height: end as u64,
                hash: vec![],
            }),
            spam_filter_threshold: 0,
        }))
        .await?
        .into_inner();
    let mut prev_hash = *prev_hash;
    let mut sap_position = get_tree_size(&tree_state.sapling_tree).unwrap();
    let mut orc_position = get_tree_size(&tree_state.orchard_tree).unwrap();

    let mut txs = vec![];
    while let Ok(Some(block)) = blocks.message().await {
        let block_prev_hash: Hash = block.prev_hash.try_into().unwrap();
        if prev_hash != block_prev_hash {
            println!("{} {}", block.height, hex::encode(block_prev_hash));
            anyhow::bail!("Reorg detected");
        }
        prev_hash = block.hash.try_into().unwrap();

        for vtx in block.vtx.iter() {
            let mut found = false;
            if let Some(sap_dec) = &sap_dec {
                for o in vtx.outputs.iter() {
                    if sap_dec.try_compact_note_decryption(o) {
                        found = true;
                    }
                }
            }

            if let Some(orc_dec) = &orc_dec {
                for o in vtx.actions.iter() {
                    if orc_dec.try_compact_note_decryption(o) {
                        found = true;
                    }
                }
            }

            if found {
                let tx = WalletTx {
                    txid: vtx.hash.clone().try_into().unwrap(),
                    sap_position,
                    orc_position,
                };
                println!("{tx:?}");
                txs.push(tx);
            }

            sap_position += vtx.outputs.len() as u32;
            orc_position += vtx.actions.len() as u32;
        }
    }

    Ok(txs)
}

pub fn get_tree_size(tree: &str) -> Result<u32> {
    let tree = hex::decode(tree)?;
    if tree.is_empty() { return Ok(0); }
    let tree = read_commitment_tree::<DummyNode, _, 32>(&*tree)?;

    Ok(tree.size() as u32)
}

pub trait Pool {
    type PreparedIncomingViewingKey;
    type Output;
}

pub struct Sapling;

impl Pool for Sapling {
    type PreparedIncomingViewingKey = sapling_crypto::keys::PreparedIncomingViewingKey;
    type Output = CompactSaplingOutput;
}

pub trait Decode<P: Pool> {
    fn try_compact_note_decryption(&self, output: &P::Output) -> bool;
}

pub struct Decoder<P: Pool> {
    pub pivk: P::PreparedIncomingViewingKey,
}

impl<P: Pool> Decoder<P> {
    pub fn new(pivk: P::PreparedIncomingViewingKey) -> Self {
        Self { pivk }
    }
}

impl Decode<Sapling> for Decoder<Sapling> {
    fn try_compact_note_decryption(&self, output: &CompactSaplingOutput) -> bool {
        let domain = SaplingDomain::new(Zip212Enforcement::On);
        try_compact_note_decryption(&domain, &self.pivk, output).is_some()
    }
}

pub struct Orchard;

impl Pool for Orchard {
    type PreparedIncomingViewingKey = orchard::keys::PreparedIncomingViewingKey;
    type Output = CompactOrchardAction;
}

impl Decode<Orchard> for Decoder<Orchard> {
    fn try_compact_note_decryption(&self, action: &CompactOrchardAction) -> bool {
        let epk: &[u8; 32] = action.ephemeral_key.as_slice().try_into().unwrap();
        let ca = CompactAction::from_parts(
            Nullifier::from_bytes(action.nullifier.as_slice().try_into().unwrap()).unwrap(),
            ExtractedNoteCommitment::from_bytes(action.cmx.as_slice().try_into().unwrap()).unwrap(),
            EphemeralKeyBytes(*epk),
            action.ciphertext.as_slice().try_into().unwrap(),
        );
        let domain = OrchardDomain::for_compact_action(&ca);
        try_compact_note_decryption(&domain, &self.pivk, &ca).is_some()
    }
}

// We don't need to know the commitment tree nodes because we are not
// making transactions. However, we have to pretend to read it so that
// we know how many nodes were used and derive the *position* of the
// notes we receive
pub struct DummyNode;

impl HashSer for DummyNode {
    fn read<R: std::io::Read>(mut reader: R) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        let mut buf = [0u8; 32];
        reader.read_exact(&mut buf)?;
        Ok(DummyNode {})
    }

    fn write<W: std::io::Write>(&self, _writer: W) -> std::io::Result<()> {
        unreachable!()
    }
}

impl ShieldedOutput<SaplingDomain, 52> for CompactSaplingOutput {
    fn ephemeral_key(&self) -> EphemeralKeyBytes {
        let hash: Hash = self.epk.clone().try_into().unwrap();
        EphemeralKeyBytes::from(hash)
    }

    fn cmstar_bytes(
        &self,
    ) -> <SaplingDomain as zcash_note_encryption::Domain>::ExtractedCommitmentBytes {
        let hash: Hash = self.cmu.clone().try_into().unwrap();
        hash
    }

    fn enc_ciphertext(&self) -> &[u8; 52] {
        self.ciphertext.as_slice().try_into().unwrap()
    }
}

#[derive(Debug)]
pub struct WalletTx {
    pub txid: Hash,
    pub sap_position: u32,
    pub orc_position: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    const FVK: &str = "uview1s5ranpd74zd2pseylw0fmt0cnudf9765mwjjd9mqf8tvjq2nlw9vgypzqayfvs7aeedguwl4r7exz50nrw6llfs3n9xfd4sm2slaay7smysc4yjyuwu3z7n5ccvyw70qkw28yt6xwra6c8d20ewpjeqq4enmftyly3fmn78hwwkyffp2y4x2vk8050vcly8y5fuse5s9e5j4wmwuldemxahrp4zrgatj63mnpqlpacvcudqfsm5ee29pj8lr5wt93eyrx3fwa64m6505cge6n46c7eqw59e0n3m9rmsntcflfmu9wyjgfk2pmjf4npkml93vyq0fps2rh4mdwpz4ld059m6mamjht99j7sdypwx52lj6lvrfgwja4uf7qy2g8d6gkmvkh7u4dksq5gazxvye4gtwfgwmuygg2sqmkkf4fjd3ymf0mq99rhf0trsl0lpddw64r4n7jj7mxy6fcpj64vkx0pre2lla9p8nknrt2c33zy3vaczd";

    #[tokio::test]
    async fn test() -> Result<()> {
        let prev_hash =
            hex::decode("5f03d35ae940bb840564c3b7af7ab72255096d3eca15c910c0e40d0000000000")
                .unwrap();
        scan(
            &Network::Main,
            "https://zec.rocks",
            2_890_000,
            2_900_000,
            &prev_hash.try_into().unwrap(),
            &UnifiedFullViewingKey::decode(&Network::Main, FVK).unwrap(),
        )
        .await?;

        Ok(())
    }
}
