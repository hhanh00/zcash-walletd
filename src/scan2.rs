use std::collections::HashSet;

use anyhow::Result;
use orchard::{
    keys::FullViewingKey,
    note::{ExtractedNoteCommitment, Nullifier},
    note_encryption::{CompactAction, OrchardDomain},
    primitives::redpallas::{Signature, SpendAuth},
    Action,
};
use sapling_crypto::{
    bundle::OutputDescription,
    note_encryption::{SaplingDomain, Zip212Enforcement},
    NullifierDerivingKey,
};
use tonic::{transport::Channel, Request};
use zcash_address::unified::{self, Encoding};
use zcash_keys::encoding::AddressCodec;
use zcash_note_encryption::{
    try_compact_note_decryption, try_note_decryption, EphemeralKeyBytes, ShieldedOutput,
};
use zcash_primitives::{
    merkle_tree::{read_commitment_tree, HashSer},
    transaction::Transaction,
};
use zcash_protocol::{
    consensus::{BlockHeight, BranchId, Parameters},
    memo::{Memo, MemoBytes},
};

use crate::{
    lwd_rpc::{
        compact_tx_streamer_client::CompactTxStreamerClient, BlockId, BlockRange,
        CompactOrchardAction, CompactSaplingOutput, TxFilter,
    },
    network::Network,
};

pub type Hash = [u8; 32];
pub type Client = CompactTxStreamerClient<Channel>;

pub async fn scan(
    network: &Network,
    client: &mut Client,
    start: u32,
    end: u32,
    prev_hash: &Hash,
    sap_dec: &mut Option<Decoder<Sapling>>,
    orc_dec: &mut Option<Decoder<Orchard>>,
) -> Result<Vec<ScanEvent>> {
    let tree_state = client
        .get_tree_state(Request::new(BlockId {
            height: start as u64,
            hash: vec![],
        }))
        .await?
        .into_inner();

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

    let mut events = vec![];
    let mut new_txids = vec![];
    while let Ok(Some(block)) = blocks.message().await {
        let height = block.height as u32;
        let block_prev_hash: Hash = block.prev_hash.try_into().unwrap();
        if prev_hash != block_prev_hash {
            println!("{} {}", block.height, hex::encode(block_prev_hash));
            anyhow::bail!("Reorg detected");
        }
        prev_hash = block.hash.try_into().unwrap();

        for vtx in block.vtx.iter() {
            let mut found = false;
            if let Some(sap_dec) = sap_dec {
                for i in vtx.spends.iter() {
                    let nf: &Hash = i.nf.as_slice().try_into().unwrap();
                    if sap_dec.nfs.contains(nf) {
                        events.push(ScanEvent::Spent(SpentNote { nf: *nf }));
                    }
                }

                for o in vtx.outputs.iter() {
                    if let Some(n) = sap_dec.try_compact_note_decryption(
                        network,
                        height,
                        &vtx.hash,
                        sap_position,
                        o,
                    )? {
                        sap_dec.add_nf(n.nf);
                        events.push(ScanEvent::Received(n));
                        found = true;
                    }
                }
            }

            if let Some(orc_dec) = orc_dec {
                for a in vtx.actions.iter() {
                    let nf: &Hash = a.nullifier.as_slice().try_into().unwrap();
                    if orc_dec.nfs.contains(nf) {
                        events.push(ScanEvent::Spent(SpentNote { nf: *nf }));
                    }
                    if let Some(n) = orc_dec.try_compact_note_decryption(
                        network,
                        height,
                        &vtx.hash,
                        orc_position,
                        a,
                    )? {
                        orc_dec.add_nf(n.nf);
                        events.push(ScanEvent::Received(n));
                        found = true;
                    }
                }
            }

            if found {
                let txid: Hash = vtx.hash.clone().try_into().unwrap();
                new_txids.push(WalletTx {
                    height,
                    txid,
                    sap_position,
                    orc_position,
                });
            }

            sap_position += vtx.outputs.len() as u32;
            orc_position += vtx.actions.len() as u32;
        }
    }

    for wtx in new_txids.iter() {
        let memos = scan_tx(network, client, wtx, sap_dec, orc_dec).await?;
        for m in memos {
            events.push(ScanEvent::Memo(m));
        }
    }

    Ok(events)
}

pub async fn scan_tx(
    network: &Network,
    client: &mut Client,
    wtx: &WalletTx,
    sap_dec: &Option<Decoder<Sapling>>,
    orc_dec: &Option<Decoder<Orchard>>,
) -> Result<Vec<MemoNote>> {
    let mut notes = vec![];
    let raw_tx = client
        .get_transaction(Request::new(TxFilter {
            hash: wtx.txid.to_vec(),
            ..TxFilter::default()
        }))
        .await?
        .into_inner();
    let branch_id = BranchId::for_height(network, BlockHeight::from_u32(wtx.height));
    let tx = Transaction::read(&*raw_tx.data, branch_id)?;
    let tx = tx.into_data();

    if let Some(sap_dec) = sap_dec {
        if let Some(sapling_bundle) = tx.sapling_bundle() {
            for (vout, o) in sapling_bundle.shielded_outputs().iter().enumerate() {
                if let Some(note) =
                    sap_dec.try_note_decryption(vout as u32 + wtx.sap_position, o)?
                {
                    notes.push(note);
                }
            }
        }
    }
    if let Some(orc_dec) = orc_dec {
        if let Some(orchard_bundle) = tx.orchard_bundle() {
            for (vout, a) in orchard_bundle.actions().iter().enumerate() {
                if let Some(note) =
                    orc_dec.try_note_decryption(vout as u32 + wtx.orc_position, a)?
                {
                    notes.push(note);
                }
            }
        }
    }
    Ok(notes)
}

pub fn get_tree_size(tree: &str) -> Result<u32> {
    let tree = hex::decode(tree)?;
    if tree.is_empty() {
        return Ok(0);
    }
    let tree = read_commitment_tree::<DummyNode, _, 32>(&*tree)?;

    Ok(tree.size() as u32)
}

pub trait Pool {
    type PreparedIncomingViewingKey;
    type NullifierKey;
    type CompactOutput;
    type Output;
}

pub struct Sapling;

#[derive(Debug)]
pub struct ReceivedNote {
    pub txid: Hash,
    pub position: u32,
    pub height: u32,
    pub address: String,
    pub diversifier: [u8; 11],
    pub value: u64,
    pub rcm: Hash,
    pub nf: Hash,
    pub rho: Option<Hash>,
}

#[derive(Debug)]
pub struct MemoNote {
    pub nf: Hash,
    pub memo: String,
}

#[derive(Debug)]
pub struct SpentNote {
    pub nf: Hash,
}

#[derive(Debug)]
pub enum ScanEvent {
    Received(ReceivedNote),
    Spent(SpentNote),
    Memo(MemoNote),
}

impl Pool for Sapling {
    type PreparedIncomingViewingKey = sapling_crypto::keys::PreparedIncomingViewingKey;
    type NullifierKey = NullifierDerivingKey;
    type CompactOutput = CompactSaplingOutput;
    type Output = OutputDescription<[u8; 192]>;
}

pub trait Decode<P: Pool> {
    fn try_compact_note_decryption(
        &self,
        network: &Network,
        height: u32,
        txid: &[u8],
        position: u32,
        output: &P::CompactOutput,
    ) -> Result<Option<ReceivedNote>>;
    fn try_note_decryption(&self, position: u32, output: &P::Output) -> Result<Option<MemoNote>>;
}

pub struct Decoder<P: Pool> {
    pub nk: P::NullifierKey,
    pub pivk: P::PreparedIncomingViewingKey,
    pub nfs: HashSet<Hash>,
}

impl<P: Pool> Decoder<P> {
    pub fn new(nk: P::NullifierKey, pivk: P::PreparedIncomingViewingKey, nfs: Vec<Hash>) -> Self {
        Self { nk, pivk, nfs: nfs.into_iter().collect() }
    }

    pub fn add_nf(&mut self, nf: Hash) {
        self.nfs.insert(nf);
    }
}

impl Decode<Sapling> for Decoder<Sapling> {
    fn try_compact_note_decryption(
        &self,
        network: &Network,
        height: u32,
        txid: &[u8],
        position: u32,
        output: &CompactSaplingOutput,
    ) -> Result<Option<ReceivedNote>> {
        let domain = SaplingDomain::new(Zip212Enforcement::On);
        if let Some((note, pa)) = try_compact_note_decryption(&domain, &self.pivk, output) {
            let address = pa.encode(network);
            let diversifier = pa.diversifier().0;
            let value = note.value().inner();
            let rcm = note.rcm().to_bytes();
            let nf = note.nf(&self.nk, position as u64);
            let note = ReceivedNote {
                txid: txid.try_into().unwrap(),
                position,
                height,
                address,
                diversifier,
                value,
                rcm,
                nf: nf.to_vec().try_into().unwrap(),
                rho: None,
            };
            return Ok(Some(note));
        }
        Ok(None)
    }

    fn try_note_decryption(
        &self,
        position: u32,
        output: &OutputDescription<[u8; 192]>,
    ) -> Result<Option<MemoNote>> {
        let domain = SaplingDomain::new(Zip212Enforcement::On);
        if let Some((note, _pa, memo_bytes)) = try_note_decryption(&domain, &self.pivk, output) {
            let nf = note.nf(&self.nk, position as u64);
            let memo_note = MemoNote {
                nf: nf.0,
                memo: memo_text(&memo_bytes)?,
            };
            return Ok(Some(memo_note));
        }
        Ok(None)
    }
}

pub struct Orchard;

impl Pool for Orchard {
    type NullifierKey = FullViewingKey;
    type PreparedIncomingViewingKey = orchard::keys::PreparedIncomingViewingKey;
    type CompactOutput = CompactOrchardAction;
    type Output = Action<Signature<SpendAuth>>;
}

impl Decode<Orchard> for Decoder<Orchard> {
    fn try_compact_note_decryption(
        &self,
        network: &Network,
        height: u32,
        txid: &[u8],
        position: u32,
        action: &CompactOrchardAction,
    ) -> Result<Option<ReceivedNote>> {
        let epk: &[u8; 32] = action.ephemeral_key.as_slice().try_into().unwrap();
        let ca = CompactAction::from_parts(
            Nullifier::from_bytes(action.nullifier.as_slice().try_into().unwrap()).unwrap(),
            ExtractedNoteCommitment::from_bytes(action.cmx.as_slice().try_into().unwrap()).unwrap(),
            EphemeralKeyBytes(*epk),
            action.ciphertext.as_slice().try_into().unwrap(),
        );
        let domain = OrchardDomain::for_compact_action(&ca);
        if let Some((note, address)) = try_compact_note_decryption(&domain, &self.pivk, &ca) {
            let ua = unified::Receiver::Orchard(address.to_raw_address_bytes());
            let ua = unified::Address::try_from_items(vec![ua])?;
            let ua = ua.encode(&network.network_type());
            let diversifier = *address.diversifier().as_array();
            let value = note.value().inner();
            let rcm = *note.rseed().as_bytes();
            let nf = note.nullifier(&self.nk);
            let rho = note.rho().to_bytes();
            let note = ReceivedNote {
                txid: txid.try_into().unwrap(),
                position,
                height,
                address: ua,
                diversifier,
                value,
                rcm,
                nf: nf.to_bytes(),
                rho: Some(rho),
            };
            return Ok(Some(note));
        }
        Ok(None)
    }

    fn try_note_decryption(
        &self,
        _position: u32,
        action: &Action<Signature<SpendAuth>>,
    ) -> Result<Option<MemoNote>> {
        let domain = OrchardDomain::for_action(action);
        if let Some((note, _address, memo_bytes)) = try_note_decryption(&domain, &self.pivk, action)
        {
            let nf = note.nullifier(&self.nk);
            let memo_note = MemoNote {
                nf: nf.to_bytes(),
                memo: memo_text(&memo_bytes)?,
            };
            return Ok(Some(memo_note));
        }

        Ok(None)
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
    pub height: u32,
    pub txid: Hash,
    pub sap_position: u32,
    pub orc_position: u32,
}

pub fn memo_text(memo_bytes: &[u8]) -> Result<String> {
    let memo_bytes = MemoBytes::from_bytes(memo_bytes)?;
    let memo = Memo::try_from(memo_bytes)?;
    let memo = if let Memo::Text(memo) = memo {
        memo.to_string()
    } else {
        String::new()
    };
    Ok(memo)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    const FVK: &str = "uview1s5ranpd74zd2pseylw0fmt0cnudf9765mwjjd9mqf8tvjq2nlw9vgypzqayfvs7aeedguwl4r7exz50nrw6llfs3n9xfd4sm2slaay7smysc4yjyuwu3z7n5ccvyw70qkw28yt6xwra6c8d20ewpjeqq4enmftyly3fmn78hwwkyffp2y4x2vk8050vcly8y5fuse5s9e5j4wmwuldemxahrp4zrgatj63mnpqlpacvcudqfsm5ee29pj8lr5wt93eyrx3fwa64m6505cge6n46c7eqw59e0n3m9rmsntcflfmu9wyjgfk2pmjf4npkml93vyq0fps2rh4mdwpz4ld059m6mamjht99j7sdypwx52lj6lvrfgwja4uf7qy2g8d6gkmvkh7u4dksq5gazxvye4gtwfgwmuygg2sqmkkf4fjd3ymf0mq99rhf0trsl0lpddw64r4n7jj7mxy6fcpj64vkx0pre2lla9p8nknrt2c33zy3vaczd";

    #[tokio::test]
    async fn test() -> Result<()> {
        let mut client = CompactTxStreamerClient::connect("https://zec.rocks".to_string()).await?;

        let prev_hash =
            hex::decode("5f03d35ae940bb840564c3b7af7ab72255096d3eca15c910c0e40d0000000000")
                .unwrap();
        let ufvk = zcash_keys::keys::UnifiedFullViewingKey::decode(&Network::Main, FVK).unwrap();
        let mut sap_dec = ufvk.sapling().map(|fvk| {
            let nk = fvk.fvk().vk.nk;
            let ivk = fvk.to_ivk(zcash_primitives::zip32::Scope::External);
            let pivk = sapling_crypto::keys::PreparedIncomingViewingKey::new(&ivk);
            Decoder::<Sapling>::new(nk, pivk, vec![])
        });
        let mut orc_dec = ufvk.orchard().map(|fvk| {
            let ivk = fvk.to_ivk(zcash_primitives::zip32::Scope::External);
            let pivk = orchard::keys::PreparedIncomingViewingKey::new(&ivk);
            Decoder::<Orchard>::new(fvk.clone(), pivk, vec![])
        });

        let events = scan(
            &Network::Main,
            &mut client,
            2_890_000,
            2_900_000,
            &prev_hash.try_into().unwrap(),
            &mut sap_dec,
            &mut orc_dec,
        )
        .await?;

        println!("{events:?}");

        Ok(())
    }
}
