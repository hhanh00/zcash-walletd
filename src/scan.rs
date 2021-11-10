use crate::lwd_rpc::compact_tx_streamer_client::CompactTxStreamerClient;
use tonic::Request;
use zcash_primitives::zip32::ExtendedFullViewingKey;
use zcash_primitives::transaction::components::sapling::CompactOutputDescription;
use crate::NETWORK;
use zcash_primitives::sapling::note_encryption::{try_sapling_compact_note_decryption, try_sapling_note_decryption};
use zcash_primitives::consensus::{BlockHeight, Parameters};
use zcash_primitives::sapling::{SaplingIvk, Node, ViewingKey};
use group::GroupEncoding;
use ff::PrimeField;
use zcash_primitives::merkle_tree::CommitmentTree;
use zcash_client_backend::encoding::encode_payment_address;
use tokio::sync::mpsc::{Sender, channel};
use zcash_primitives::transaction::{TxId, Transaction};
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::Stream;
use tonic::transport::Channel;
use zcash_primitives::memo::Memo;
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

pub struct Block {
    pub height: u32,
    pub hash: [u8; 32],
}

pub async fn get_latest_height(client: &mut CompactTxStreamerClient<Channel>) -> anyhow::Result<u32> {
    let latest_block_id = client.get_latest_block(Request::new(ChainSpec {})).await?.into_inner();
    let latest_height = latest_block_id.height;
    Ok(latest_height as u32)
}

pub async fn scan_blocks(start_height: u32, lwd_url: &str, fvk: &ExtendedFullViewingKey, mut prev_block_hash: Option<[u8; 32]>) -> anyhow::Result<(impl Stream<Item=TxIndex>, BoxFuture<'static, anyhow::Result<Option<Block>>>)> {
    let mut client = CompactTxStreamerClient::connect(lwd_url.to_string()).await?;
    let latest_height = get_latest_height(&mut client).await?;
    let start_block_id = BlockId {
        height: start_height as u64,
        hash: vec![],
    };
    let (tx, rx) = channel::<TxIndex>(1);
    if start_height <= latest_height {
        let tree_state = client.get_tree_state(Request::new(start_block_id)).await?.into_inner();
        let commitment_tree = hex::decode(tree_state.tree)?;
        let commitment_tree = CommitmentTree::<Node>::read(&*commitment_tree)?;
        let mut current_position = commitment_tree.size();
        println!("Scanning from {} to {}", start_height, latest_height);
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
            let mut last_block: Option<Block> = None;
            while let Some(block) = block_stream.message().await? {
                let prev_block_hash = prev_block_hash.take();
                if let Some(prev_block_hash) = prev_block_hash {
                    println!("Chaintip check");
                    println!("{} {}", hex::encode(&prev_block_hash), hex::encode(&block.prev_hash));
                    if prev_block_hash.to_vec() != block.prev_hash {
                        println!("Chaintip check");
                        return Err(ScanError::Reorganization.into());
                    }
                }
                let count_notes = scan_one_block(&block, &fvk2, current_position, &tx).await?;
                current_position += count_notes;
                let mut b = Block {
                    height: block.height as u32,
                    hash: [0u8; 32],
                };
                b.hash.copy_from_slice(&block.hash);
                last_block = Some(b);
            }
            println!("SCAN FINISHED");
            Ok::<_, anyhow::Error>(last_block)
        });

        Ok((ReceiverStream::new(rx), Box::pin(jh.map(|e| e?))))
    }
    else {
        Ok((ReceiverStream::new(rx), Box::pin(future::ok(None))))
    }
}

#[derive(Debug)]
pub struct TxIndex {
    pub height: u32,
    pub tx_id: TxId,
    pub position: usize,
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

async fn scan_one_block(block: &CompactBlock, fvk: &ExtendedFullViewingKey, start_position: usize, tx: &Sender<TxIndex>) -> anyhow::Result<usize> {
    // println!("{}", block.height);
    let vk = fvk.fvk.vk.clone();
    let ivk = vk.ivk();
    let height = BlockHeight::from_u32(block.height as u32);
    let mut count_notes = 0;
    for transaction in block.vtx.iter() {
        for cout in transaction.outputs.iter() {
            let co = to_output_description(cout);
            if try_sapling_compact_note_decryption(&NETWORK, height, &ivk, &co).is_some() {
                let mut tx_id = [0u8; 32];
                tx_id.copy_from_slice(&transaction.hash);
                let tx_index = TxIndex {
                    height: block.height as u32,
                    tx_id: TxId(tx_id),
                    position: start_position + count_notes,
                };
                println!("{}", block.height);
                tx.send(tx_index).await?;
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
    let cmu = bls12_381::Scalar::from_repr(cmu).unwrap();
    let mut epk = [0u8; 32];
    epk.copy_from_slice(&co.epk);
    let epk = jubjub::ExtendedPoint::from_bytes(&epk).unwrap();
    let od = CompactOutputDescription {
        epk,
        cmu,
        enc_ciphertext: co.ciphertext.to_vec(),
    };
    od
}

pub async fn scan_transaction(client: &mut CompactTxStreamerClient<Channel>, height: u32, tx_id: TxId,
                              tx_position: usize, vk: &ViewingKey, ivk: &SaplingIvk, nf_map: &HashMap<[u8; 32], u32>) -> anyhow::Result<(Vec<u32>, Vec<DecryptedNote>, i64)> {
    let raw_tx = client.get_transaction(Request::new(TxFilter {
        block: None,
        index: 0,
        hash: tx_id.0.to_vec(),
    })).await?.into_inner();
    let tx = Transaction::read(&*raw_tx.data)?;

    let mut spends: Vec<u32> = vec![];
    let mut outputs: Vec<DecryptedNote> = vec![];

    for sd in tx.shielded_spends.iter() {
        if let Some(id_note) = nf_map.get(&sd.nullifier.0) {
            spends.push(*id_note);
        }
    }

    for (index, od) in tx.shielded_outputs.iter().enumerate() {
        if let Some((note, pa, memo)) = try_sapling_note_decryption(&NETWORK, BlockHeight::from_u32(height), ivk, od) {
            let memo: Memo = Memo::try_from(memo)?;
            let memo = match memo {
                Memo::Text(text) => text.to_string(),
                _ => "".to_string(),
            };
            let diversifier: [u8; 11] = pa.diversifier().0;
            let rcm = note.rcm().to_repr();
            let position = tx_position + index;
            let nf = note.nf(vk, position as u64);
            let note = DecryptedNote {
                address: encode_payment_address(NETWORK.hrp_sapling_payment_address(), &pa),
                height,
                position,
                diversifier,
                value: note.value,
                rcm,
                nf: nf.0,
                memo,
            };
            outputs.push(note);
        }
    }

    println!("TXID: {}", tx.txid());
    let value = i64::from(tx.value_balance);
    Ok((spends, outputs, value))
}

#[allow(unreachable_code)]
pub async fn monitor_task(port: u16) {
    tokio::spawn(async move {
        loop {
            let client = reqwest::Client::new();
            tokio::time::sleep(Duration::from_secs(5)).await;
            client
                .post(format!("http://localhost:{}/request_scan", port))
                .json(&HashMap::<String, String>::new())
                .send().await?;
        }
        Ok::<_, anyhow::Error>(())
    });
}
