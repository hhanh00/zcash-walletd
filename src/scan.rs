use crate::lwd_rpc::compact_tx_streamer_client::CompactTxStreamerClient;
use crate::network::Network;
use sapling_crypto::keys::PreparedIncomingViewingKey;
use sapling_crypto::note::ExtractedNoteCommitment;
use sapling_crypto::note_encryption::{try_sapling_compact_note_decryption, try_sapling_note_decryption, CompactOutputDescription};
use sapling_crypto::zip32::ExtendedFullViewingKey;
use sapling_crypto::{Node, ViewingKey, NOTE_COMMITMENT_TREE_DEPTH};
use tonic::Request;
use zcash_primitives::merkle_tree::read_commitment_tree;
use zcash_primitives::transaction::components::sapling::zip212_enforcement;
use zcash_primitives::consensus::{BlockHeight, BranchId, NetworkConstants as _};
use zcash_client_backend::encoding::encode_payment_address;
use tokio::sync::mpsc::{Sender, channel};
use zcash_primitives::transaction::{TxId, Transaction};
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::Stream;
use tonic::transport::Channel;
use zcash_primitives::memo::{Memo, MemoBytes};
use std::convert::TryFrom;
use std::collections::HashMap;
use rocket::futures::{FutureExt, future};
use rocket::futures::future::BoxFuture;
use crate::lwd_rpc::{ChainSpec, BlockId, BlockRange, CompactBlock, CompactOutput, TxFilter};
use tokio::time::Duration;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ScanError {
    #[error("Blockchain Reorganization")]
    Reorganization,
}

#[derive(Debug)]
pub enum ScannerOutput {
    TxIndex(TxIndex),
    Block(Block),
}

#[derive(Debug)]
pub struct TxIndex {
    pub height: u32,
    pub tx_id: TxId,
    pub position: usize,
}

#[derive(Debug)]
pub struct Block {
    pub height: u32,
    pub hash: [u8; 32],
}

pub async fn get_latest_height(client: &mut CompactTxStreamerClient<Channel>) -> anyhow::Result<u32> {
    let latest_block_id = client.get_latest_block(Request::new(ChainSpec {})).await?.into_inner();
    let latest_height = latest_block_id.height;
    Ok(latest_height as u32)
}

pub async fn scan_blocks(network: Network, start_height: u32, lwd_url: &str, fvk: &ExtendedFullViewingKey, mut prev_block_hash: Option<[u8; 32]>)
    -> anyhow::Result<(impl Stream<Item=ScannerOutput>, BoxFuture<'static, anyhow::Result<()>>)> {
    let mut client = CompactTxStreamerClient::connect(lwd_url.to_string()).await?;
    let latest_height = get_latest_height(&mut client).await?;
    let start_block_id = BlockId {
        height: start_height as u64,
        hash: vec![],
    };
    let (scan_sender, scan_receiver) = channel::<ScannerOutput>(1);
    if start_height <= latest_height {
        let tree_state = client.get_tree_state(Request::new(start_block_id)).await?.into_inner();
        let commitment_tree = hex::decode(tree_state.tree)?;
        let commitment_tree = read_commitment_tree::<Node, _, NOTE_COMMITMENT_TREE_DEPTH>(&*commitment_tree)?;
        let mut current_position = commitment_tree.size();
        log::info!("Scanning from {} to {}", start_height, latest_height);
        let mut block_stream = client
            .get_block_range(Request::new(BlockRange {
                start: Some(BlockId {
                    height: start_height as u64,
                    hash: vec![],
                }),
                end: Some(BlockId {
                    height: latest_height as u64,
                    hash: vec![],
                }),
            }))
            .await?
            .into_inner();

        let fvk2 = fvk.clone();
        let jh = tokio::spawn(async move {
            while let Some(block) = block_stream.message().await? {
                let prev_block_hash = prev_block_hash.take();
                if let Some(prev_block_hash) = prev_block_hash {
                    if prev_block_hash.to_vec() != block.prev_hash {
                        log::info!("Chaintip mismatch");
                        return Err(ScanError::Reorganization.into());
                    }
                }
                let count_notes = scan_one_block(&network, &block, &fvk2, current_position, &scan_sender).await?;
                current_position += count_notes;
                let mut b = Block {
                    height: block.height as u32,
                    hash: [0u8; 32],
                };
                b.hash.copy_from_slice(&block.hash);
                scan_sender.send(ScannerOutput::Block(b)).await?;
            }
            log::info!("SCAN FINISHED");
            Ok::<_, anyhow::Error>(())
        });

        Ok((ReceiverStream::new(scan_receiver), Box::pin(jh.map(|e| e?))))
    }
    else {
        Ok((ReceiverStream::new(scan_receiver), Box::pin(future::ok(()))))
    }
}

pub struct DecryptedNote {
    pub address: String,
    pub height: u32,
    pub position: usize,
    pub diversifier: [u8; 11],
    pub value: u64,
    pub rcm: [u8; 32],
    pub nf: [u8; 32],
    pub memo: String,
}

async fn scan_one_block(network: &Network, block: &CompactBlock, fvk: &ExtendedFullViewingKey, start_position: usize, tx: &Sender<ScannerOutput>) -> anyhow::Result<usize> {
    // println!("{}", block.height);
    let vk = fvk.fvk.vk.clone();
    let ivk = vk.ivk();
    let pivk = PreparedIncomingViewingKey::new(&ivk);
    let height = BlockHeight::from_u32(block.height as u32);
    let mut count_notes = 0;
    let zip32_enforcement = zip212_enforcement(network, height);
    for transaction in block.vtx.iter() {
        for cout in transaction.outputs.iter() {
            let co = to_output_description(cout);
            if try_sapling_compact_note_decryption(&pivk, &co, zip32_enforcement).is_some() {
                let mut tx_id = [0u8; 32];
                tx_id.copy_from_slice(&transaction.hash);
                let tx_index = TxIndex {
                    height: block.height as u32,
                    tx_id: TxId::from_bytes(tx_id),
                    position: start_position + count_notes,
                };
                tx.send(ScannerOutput::TxIndex(tx_index)).await?;
                break;
            }
        }
        count_notes += transaction.outputs.len();
    }
    Ok(count_notes)
}

pub fn to_output_description(co: &CompactOutput) -> CompactOutputDescription {
    let mut cmu = [0u8; 32];
    cmu.copy_from_slice(&co.cmu);
    let mut epk = [0u8; 32];
    epk.copy_from_slice(&co.epk);
    let mut enc_ciphertext = [0u8; 52];
    enc_ciphertext.copy_from_slice(&co.ciphertext);
    
    CompactOutputDescription {
        ephemeral_key: epk.into(),
        cmu: ExtractedNoteCommitment::from_bytes(&cmu).unwrap(),
        enc_ciphertext,
    }
}

pub async fn scan_transaction(network: &Network, client: &mut CompactTxStreamerClient<Channel>, height: u32, tx_id: TxId,
                              tx_position: usize, vk: &ViewingKey, pivk: &PreparedIncomingViewingKey, nf_map: &HashMap<[u8; 32], u32>) -> anyhow::Result<(Vec<u32>, Vec<DecryptedNote>, i64)> {
    log::info!("Scan tx id: {}", tx_id);
    let raw_tx = client.get_transaction(Request::new(TxFilter {
        block: None,
        index: 0,
        hash: tx_id.as_ref().to_vec(),
    })).await?.into_inner();
    let branch_id = BranchId::for_height(network, BlockHeight::from_u32(height));
    let tx = Transaction::read(&*raw_tx.data, branch_id)?;
    let txid = tx.txid();
    let tx = tx.into_data();

    let zip32_enforcement = zip212_enforcement(network, BlockHeight::from_u32(height));
    let mut spends: Vec<u32> = vec![];
    let mut outputs: Vec<DecryptedNote> = vec![];

    if let Some(sapling_bundle) = tx.sapling_bundle() {
        for sd in sapling_bundle.shielded_spends() {
            if let Some(id_note) = nf_map.get(sd.nullifier().as_ref()) {
                spends.push(*id_note);
            }
        }

        for (index, od) in sapling_bundle.shielded_outputs().iter().enumerate() {
            if let Some((note, pa, memo)) = try_sapling_note_decryption(pivk, od, zip32_enforcement) {
                let memo_bytes = MemoBytes::from_bytes(&memo).unwrap();
                let memo: Memo = Memo::try_from(memo_bytes)?;
                let memo = match memo {
                    Memo::Text(text) => text.to_string(),
                    _ => "".to_string(),
                };
                let diversifier: [u8; 11] = pa.diversifier().0;
                let rcm = note.rcm().to_bytes();
                let position = tx_position + index;
                let nf = note.nf(&vk.nk, position as u64);
                let note = DecryptedNote {
                    address: encode_payment_address(network.hrp_sapling_payment_address(), &pa),
                    height,
                    position,
                    diversifier,
                    value: note.value().inner(),
                    rcm,
                    nf: nf.0,
                    memo,
                };
                outputs.push(note);
            }
        }
    }

    log::info!("TXID: {}", txid);
    let value = i64::from(tx.sapling_value_balance());
    Ok((spends, outputs, value))
}

#[allow(unreachable_code)]
pub async fn monitor_task(birth_height: Option<u32>, port: u16, poll_interval: u16) {
    let mut params = HashMap::<&str, u32>::new();
    if let Some(birth_height) = birth_height {
        params.insert("start_height", birth_height);
    }
    tokio::spawn(async move {
        loop {
            let client = reqwest::Client::new();
            client
                .post(format!("http://localhost:{}/request_scan", port))
                .json(&params)
                .send().await?;
            params.clear();

            tokio::time::sleep(Duration::from_secs(poll_interval as u64)).await;
        }
        Ok::<_, anyhow::Error>(())
    });
}
