use solana_program::keccak::hashv;
use solana_sdk::{keccak::Hash, signer::Signer};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::messages::{MineJob, SendJob};

pub struct Miner;

impl Miner {
    pub fn start(mut mine_rx: UnboundedReceiver<MineJob>, send_tx: UnboundedSender<SendJob>) {
        tokio::task::spawn(async move {
            while let Some(job) = mine_rx.recv().await {
                let send_job = Miner::mine(job);
                logfather::crit!(
                    "found hash {} for tunnel {}",
                    &send_job.hash.to_string()[..12],
                    send_job.tunnel.id
                );
                send_tx.send(send_job).ok();
            }
        });
    }

    fn mine(job: MineJob) -> SendJob {
        let mut hash: Hash;
        for nonce in 1_u64.. {
            hash = hashv(&[
                job.challenge.as_ref(),
                job.tunnel.keypair.pubkey().as_ref(),
                nonce.to_le_bytes().as_slice(),
            ]);
            if hash.le(&job.difficulty) {
                return SendJob {
                    tunnel: job.tunnel,
                    hash,
                    nonce,
                };
            }
        }
        panic!("Could not find a valid hash")
    }
}
