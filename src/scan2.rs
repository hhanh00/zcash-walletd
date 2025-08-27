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
    ufvk: UnifiedFullViewingKey,
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
        Decoder::<Sapling>::new(get_tree_size(&tree_state.sapling_tree).unwrap(), pivk)
    });
    let orc_dec = ufvk.orchard().map(|fvk| {
        let ivk = fvk.to_ivk(zcash_primitives::zip32::Scope::External);
        let pivk = orchard::keys::PreparedIncomingViewingKey::new(&ivk);
        Decoder::<Orchard>::new(get_tree_size(&tree_state.sapling_tree).unwrap(), pivk)
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
    let mut sap_position = 0;
    let mut orc_position = 0;

    let mut txs = vec![];
    while let Ok(Some(block)) = blocks.message().await {
        let block_prev_hash: Hash = block.prev_hash.try_into().unwrap();
        if prev_hash != block_prev_hash {
            anyhow::bail!("Reorg detected");
        }
        prev_hash = block_prev_hash;

        for vtx in block.vtx.iter() {
            if let Some(sap_dec) = &sap_dec {
                for o in vtx.outputs.iter() {
                    if sap_dec.try_compact_note_decryption(o) {
                        txs.push(WalletTx {
                            txid: vtx.hash.clone().try_into().unwrap(),
                            sap_position,
                            orc_position,
                        });
                    }
                }
            }

            if let Some(orc_dec) = &orc_dec {
                for o in vtx.actions.iter() {
                    if orc_dec.try_compact_note_decryption(o) {
                        txs.push(WalletTx {
                            txid: vtx.hash.clone().try_into().unwrap(),
                            sap_position,
                            orc_position,
                        });
                    }
                }
            }
            sap_position += vtx.outputs.len() as u32;
            orc_position += vtx.actions.len() as u32;
        }
    }

    Ok(txs)
}

pub fn get_tree_size(tree: &str) -> Result<u32> {
    let tree = hex::decode(tree)?;
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
    pub position: u32,
    pub pivk: P::PreparedIncomingViewingKey,
}

impl<P: Pool> Decoder<P> {
    pub fn new(position: u32, pivk: P::PreparedIncomingViewingKey) -> Self {
        Self { position, pivk }
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

pub struct WalletTx {
    pub txid: Hash,
    pub sap_position: u32,
    pub orc_position: u32,
}
