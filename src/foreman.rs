use std::sync::Arc;

use logfather::{crit, trace};
use ore::{state::Proof, utils::AccountDeserialize};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_program::{native_token::LAMPORTS_PER_SOL, system_instruction};
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction,
    signature::{read_keypair_file, write_keypair_file, Keypair},
    signer::Signer,
    transaction::Transaction,
};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use crate::{
    blockhash::{BlockhashService, LatestBlockhash},
    messages::{MineJob, SendJob, Tunnel},
    miner::Miner,
    sender::Sender,
    treasury::{LatestTreasury, TreasuryService},
    utils::{sleep_ms, tunnel_keypair_filepath},
};

pub struct Foreman {
    rpc: Arc<RpcClient>,
    keypair: Keypair,
    blockhash: LatestBlockhash,
    treasury: LatestTreasury,
    tunnel_rx: UnboundedReceiver<Tunnel>,
    mine_txs: Vec<UnboundedSender<MineJob>>,
    priority_fee: u64,
}

impl Foreman {
    pub async fn start(
        keypair_filepath: String,
        rpc_url: String,
        priority_fee: u64,
        tip_amount: u64,
        num_tunnels: u64,
    ) {
        // Fetch keypair
        let keypair = read_keypair_file(keypair_filepath.clone()).expect("failed to read keypair");

        // Start network services
        let rpc = Arc::new(RpcClient::new(rpc_url));
        let blockhash = BlockhashService::start(rpc.clone());
        let treasury = TreasuryService::start(rpc.clone());

        // Initialize tunnels
        let mut mine_txs = vec![];
        let (tunnel_tx, tunnel_rx) = mpsc::unbounded_channel::<Tunnel>();
        let (send_tx, send_rx) = mpsc::unbounded_channel::<SendJob>();
        for i in 0..num_tunnels {
            // Get or create tunnel keypair
            let kp_path = tunnel_keypair_filepath(i).expect("Failed to get keypair filepath");
            let kp = match read_keypair_file(kp_path.clone()) {
                Ok(kp) => kp,
                Err(_) => {
                    let kp = Keypair::new();
                    write_keypair_file(&kp, kp_path).expect("Failed to create keypair file");
                    kp
                }
            };

            // Initialize tunnel
            let tunnel = Tunnel::new(kp, i as usize);
            tunnel_tx.send(tunnel).ok();

            // Start miner
            let (mine_tx, mine_rx) = mpsc::unbounded_channel::<MineJob>();
            Miner::start(mine_rx, send_tx.clone());
            mine_txs.push(mine_tx);
        }

        // Start sender
        Sender::start(
            rpc.clone(),
            keypair,
            send_rx,
            tunnel_tx.clone(),
            blockhash.clone(),
            priority_fee,
            tip_amount,
        );

        // Run
        let keypair = read_keypair_file(keypair_filepath).expect("failed to read keypair");
        Foreman {
            rpc,
            keypair,
            blockhash,
            treasury,
            tunnel_rx,
            mine_txs,
            priority_fee,
        }
        .run()
        .await;
    }

    async fn run(&mut self) {
        while let Some(tunnel) = self.tunnel_rx.recv().await {
            self.top_up(&tunnel).await;
            self.send_job(tunnel).await;
        }
    }

    async fn send_job(&mut self, tunnel: Tunnel) {
        // Fetch data
        let data = if let Ok(data) = self.rpc.get_account_data(&tunnel.proof).await {
            data
        } else {
            self.register(&tunnel).await;
            self.rpc
                .get_account_data(&tunnel.proof)
                .await
                .expect("failed to fetch account")
        };

        // Update arc
        let proof = Proof::try_from_bytes(&data).expect("failed to parse proof");
        let Some(treasury) = self.treasury.load().await else {
            // TODO
            return;
        };

        // Send job to miner
        self.mine_txs[tunnel.id]
            .send(MineJob {
                tunnel,
                challenge: proof.hash.into(),
                difficulty: treasury.difficulty.into(),
            })
            .ok();
    }

    async fn register(&mut self, tunnel: &Tunnel) {
        loop {
            let cu_budget_ix = ComputeBudgetInstruction::set_compute_unit_limit(50_000);
            let cu_price_ix = ComputeBudgetInstruction::set_compute_unit_price(self.priority_fee);
            let Some(blockhash) = self.blockhash.load().await else {
                continue;
            };
            let ix = ore::instruction::register(tunnel.keypair.pubkey());
            let tx = Transaction::new_signed_with_payer(
                &[cu_budget_ix, cu_price_ix, ix],
                Some(&tunnel.keypair.pubkey()),
                &[&tunnel.keypair],
                blockhash,
            );
            trace!("registering {}", tunnel.keypair.pubkey());
            if let Err(e) = self.rpc.send_and_confirm_transaction(&tx).await {
                trace!("failed to register {}: {e:#?}", tunnel.keypair.pubkey());
                sleep_ms(2000).await;
            } else {
                trace!("registered miner {}", tunnel.keypair.pubkey());
                return;
            }
        }
    }

    async fn top_up(&mut self, tunnel: &Tunnel) {
        loop {
            // Fetch balance
            let balance = self
                .rpc
                .get_balance(&tunnel.keypair.pubkey())
                .await
                .unwrap_or(0);

            // Top up if balance is below min
            const MIN_BALANCE: u64 = LAMPORTS_PER_SOL / 32;
            if balance < MIN_BALANCE {
                let amount = 2 * MIN_BALANCE - balance;
                let cu_budget_ix = ComputeBudgetInstruction::set_compute_unit_limit(1000);
                let cu_price_ix =
                    ComputeBudgetInstruction::set_compute_unit_price(self.priority_fee);
                let Some(blockhash) = self.blockhash.load().await else {
                    continue;
                };
                let ix = system_instruction::transfer(
                    &self.keypair.pubkey(),
                    &tunnel.keypair.pubkey(),
                    amount,
                );
                let tx = Transaction::new_signed_with_payer(
                    &[cu_budget_ix, cu_price_ix, ix],
                    Some(&self.keypair.pubkey()),
                    &[&self.keypair],
                    blockhash,
                );
                trace!("topping up {}", tunnel.keypair.pubkey());
                if let Err(e) = self.rpc.send_and_confirm_transaction(&tx).await {
                    trace!("failed to top up {}: {e:#?}", tunnel.keypair.pubkey());
                    sleep_ms(2000).await;
                } else {
                    trace!("topped up {}", tunnel.keypair.pubkey());
                    return;
                }
            } else {
                break;
            }
        }
    }
}
