use std::{str::FromStr, sync::Arc};

use logfather::{crit, error, trace, warn};
use ore::{state::Proof, utils::AccountDeserialize, BUS_ADDRESSES, BUS_COUNT};
use rand::Rng;
use serde_json::json;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_program::{instruction::Instruction, pubkey::Pubkey};
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction, hash::Hash, signature::Keypair, signer::Signer,
    transaction::Transaction,
};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::{
    blockhash::LatestBlockhash,
    messages::{SendJob, Tunnel},
    utils::sleep_ms,
};

pub struct Sender {
    rpc: Arc<RpcClient>,
    keypair: Keypair,
    client: Arc<reqwest::Client>,
    tunnel_tx: UnboundedSender<Tunnel>,
    blockhash: LatestBlockhash,
    priority_fee: u64,
    tip_amount: u64,
}

// TODO This implicitly assumes

const JITO_URL: &str = "https://mainnet.block-engine.jito.wtf/api/v1/bundles";
const CU_LIMIT_MINE: u32 = 10_000; // 160_000; // 3200;
const BATCH_SIZE: usize = 2;
const BUNDLE_SIZE: usize = 1; // 1;
const CONFIRMATION_DELAY: u64 = 8000;

impl Sender {
    pub fn start(
        rpc: Arc<RpcClient>,
        keypair: Keypair,
        mut send_rx: UnboundedReceiver<SendJob>,
        tunnel_tx: UnboundedSender<Tunnel>,
        blockhash: LatestBlockhash,
        priority_fee: u64,
        tip_amount: u64,
    ) {
        let sender = Arc::new(Sender {
            rpc,
            keypair,
            client: Arc::new(reqwest::Client::new()),
            tunnel_tx,
            blockhash,
            priority_fee,
            tip_amount,
        });
        tokio::task::spawn(async move {
            let mut jobs = vec![];
            while let Some(job) = send_rx.recv().await {
                trace!("received send job: {:?}", job.hash);
                jobs.push(job);
                if jobs.len().ge(&(BATCH_SIZE * BUNDLE_SIZE)) {
                    let sender = sender.clone();
                    let jobs_to_send = jobs.drain(..).collect::<Vec<_>>();
                    tokio::task::spawn(async move {
                        sender.bundle_and_send(jobs_to_send).await;
                    });
                }
            }
        });
    }

    async fn bundle_and_send(&self, jobs: Vec<SendJob>) {
        // Bundle and send
        let bundle = self.build_bundle(&jobs).await;
        self.send_transactions_as_bundle(&bundle).await;

        // With bundles, we can assume all landed if the first one landed
        loop {
            sleep_ms(CONFIRMATION_DELAY).await;
            if self.confirm(&jobs[0]).await {
                crit!("confirmed bundle!");
                break;
            } else {
                warn!("failed to confirm bundle...");
            }
        }

        // Requeue all tunnels
        for job in jobs {
            self.tunnel_tx.send(job.tunnel).ok();
        }
    }

    async fn build_bundle(&self, jobs: &[SendJob]) -> Vec<Transaction> {
        let blockhash = self
            .blockhash
            .load()
            .await
            .expect("failed to get latest blockhash");
        let mut bundle = vec![];
        for j in jobs.chunks(BATCH_SIZE) {
            let tx = self
                .build_mine_transaction(j, bundle.is_empty(), blockhash)
                .await;
            bundle.push(tx);
        }
        bundle
    }

    async fn build_mine_transaction(
        &self,
        jobs: &[SendJob],
        include_tip: bool,
        blockhash: Hash,
    ) -> Transaction {
        let cu_budget_ix = ComputeBudgetInstruction::set_compute_unit_limit(CU_LIMIT_MINE);
        let cu_price_ix = ComputeBudgetInstruction::set_compute_unit_price(self.priority_fee);
        let mut keypairs = vec![&self.keypair];
        let mut ixs = vec![cu_budget_ix, cu_price_ix];
        let mut rng = rand::thread_rng();
        let bus_id = rng.gen_range(0..BUS_COUNT);
        for job in jobs.iter() {
            keypairs.push(&*job.tunnel.keypair);
            ixs.push(ore::instruction::mine(
                job.tunnel.keypair.pubkey(),
                BUS_ADDRESSES[bus_id],
                job.hash.into(),
                job.nonce,
            ));
        }
        if include_tip {
            ixs.push(self.build_tip_instruction());
        }
        let mut tx = Transaction::new_with_payer(&ixs, Some(&self.keypair.pubkey()));
        tx.sign(&keypairs, blockhash);
        tx
    }

    fn build_tip_instruction(&self) -> Instruction {
        let mut rng = rand::thread_rng();
        let tip_accounts = &[
            Pubkey::from_str("96gYZGLnJYVFmbjzopPSU6QiEV5fGqZNyN9nmNhvrZU5").unwrap(),
            Pubkey::from_str("HFqU5x63VTqvQss8hp11i4wVV8bD44PvwucfZ2bU7gRe").unwrap(),
            Pubkey::from_str("Cw8CFyM9FkoMi7K7Crf6HNQqf4uEMzpKw6QNghXLvLkY").unwrap(),
            Pubkey::from_str("ADaUMid9yfUytqMBgopwjb2DTLSokTSzL1zt6iGPaS49").unwrap(),
            Pubkey::from_str("DfXygSm4jCyNCybVYYK6DwvWqjKee8pbDmJGcLWNDXjh").unwrap(),
            Pubkey::from_str("ADuUkR4vqLUMWXxW9gh6D6L8pMSawimctcNZ5pGwDcEt").unwrap(),
            Pubkey::from_str("DttWaMuVvTiduZRnguLF7jNxTgiMBZ1hyAumKUiL2KRL").unwrap(),
            Pubkey::from_str("3AVi9Tg9Uo68tJfuvoKvqKNWKkC5wPdSSdeBnizKZ6jT").unwrap(),
        ];
        let i = rng.gen_range(0..tip_accounts.len());
        solana_sdk::system_instruction::transfer(
            &self.keypair.pubkey(),
            &tip_accounts[i],
            self.tip_amount,
        )
    }

    async fn confirm(&self, job: &SendJob) -> bool {
        // Fetch data
        let Ok(data) = self.rpc.get_account_data(&job.tunnel.proof).await else {
            warn!("failed to fetch proof ({})", job.tunnel.proof);
            return false;
        };

        // Check if total hashes has incremented
        if let Ok(proof) = Proof::try_from_bytes(&data) {
            if proof.total_hashes.gt(&job.total_hashes) {
                return true;
            }
        }

        return false;
    }

    async fn send_transactions_as_bundle(&self, transactions: &[Transaction]) {
        // Serialize tx to bs58
        let tx_strs = transactions
            .iter()
            .map(|transaction| bs58::encode(bincode::serialize(transaction).unwrap()).into_string())
            .collect::<Vec<_>>();
        for tx in transactions {
            trace!("tx: {}", tx.signatures[0]);
        }

        match self
            .client
            .post(JITO_URL)
            .json(&json! {{
                "jsonrpc": "2.0",
                "id": "1",
                "method": "sendBundle",
                "params": [tx_strs],
            }})
            .send()
            .await
        {
            Ok(r) => {
                if let Ok(Ok(response_json)) =
                    r.text().await.map(|x| serde_json::Value::from_str(&x))
                {
                    if let Some(bundlehash) = response_json.get("result") {
                        // If submitted:
                        crit!("submitted bundle {}", bundlehash.to_string());
                    } else if let Some(error_message) =
                        response_json.get("error").and_then(|e| e.get("message"))
                    {
                        // If error
                        crit!("error: {error_message}");
                    } else {
                        crit!("{:?}", response_json);
                    }
                }
            }

            Err(e) => {
                error!("{e}");
            }
        };
    }
}
