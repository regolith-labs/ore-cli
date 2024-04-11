use std::{fs, io, path::PathBuf, sync::Arc, time::Duration};

use cached::proc_macro::cached;
use clap::{command, Parser, Subcommand};
use ore::{
    state::{Proof, Treasury},
    utils::AccountDeserialize,
    PROOF, TREASURY_ADDRESS,
};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_program::{
    keccak::{hashv, Hash},
    pubkey::Pubkey,
};
use solana_sdk::{
    signature::{read_keypair_file, write_keypair_file, Keypair},
    signer::Signer,
};
use tokio::{
    sync::{
        mpsc::{self, UnboundedReceiver, UnboundedSender},
        Mutex, RwLock,
    },
    task::JoinHandle,
    time,
};

#[derive(Parser, Debug)]
#[command(about, version)]
struct Args {
    #[arg(
        long,
        value_name = "RPC_URL",
        help = "Network address of your RPC provider",
        global = true
    )]
    rpc: Option<String>,

    #[arg(
        long,
        value_name = "KEYPAIR_FILEPATH",
        help = "Filepath to keypair to use",
        global = true
    )]
    pub keypair: Option<String>,

    #[arg(
        long,
        value_name = "MICROLAMPORTS",
        help = "Number of microlamports to pay as priority fee per transaction",
        default_value = "0",
        global = true
    )]
    pub priority_fee: u64,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    #[command(about = "Mine Ore using local compute")]
    Mine(MineArgs),
}

#[derive(Parser, Debug)]
struct MineArgs {
    #[arg(
        long,
        value_name = "TUNNELS",
        help = "The number of tunnels to mine",
        default_value = "1"
    )]
    tunnels: u64,

    #[arg(
        long,
        value_name = "TIP",
        help = "The amount to pay in Jito tips (lamports)",
        default_value = "1"
    )]
    tip: u64,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    match args.command {
        Commands::Mine(mine_args) => {
            Foreman::new(
                args.keypair.expect("Expected keypair"),
                args.rpc.expect("Expected rpc"),
                mine_args.tip,
                mine_args.tunnels,
            )
            .start()
            .await;
        }
    }
}

// struct Miner {
//     // pub keypair_filepath: Option<String>,
//     // pub priority_fee: u64,
//     // pub rpc_client: Arc<RpcClient>,
//     pub foreman: Foreman,
//     // pub tunnel
// }

struct Foreman {
    // TODO Main keypair
    // TODO Tunnels channel
    // TODO Latest treasury
    pub rpc: Arc<RpcClient>,
    pub tunnels: Vec<Arc<Tunnel>>,
    pub tunnel_rx: UnboundedReceiver<Arc<Tunnel>>,
    pub mine_tx: UnboundedSender<MineJob>,
    pub _mine_h: JoinHandle<()>,
    pub _send_h: JoinHandle<()>,
}

impl Foreman {
    pub fn new(
        kp_filepath: String,
        rpc_url: String,
        tip_amount: u64,
        tunnels_count: u64,
    ) -> Foreman {
        let _kp = read_keypair_file(kp_filepath.clone()).unwrap();
        let rpc = Arc::new(RpcClient::new(rpc_url));
        let (send_tx, send_rx) = mpsc::unbounded_channel::<SendJob>();
        let (tunnel_tx, tunnel_rx) = mpsc::unbounded_channel::<Arc<Tunnel>>();
        let (mine_tx, mut mine_rx) = mpsc::unbounded_channel::<MineJob>();
        let mut tunnels = vec![];
        for i in 0..tunnels_count {
            let tunnel = Arc::new(Tunnel::new(i, send_tx.clone()));
            tunnels.push(tunnel.clone());
            tunnel_tx.send(tunnel).ok();
        }
        let mine_h = tokio::task::spawn(async move {
            while let Some(mine_job) = mine_rx.recv().await {
                tunnels[mine_job.tunnel_id as usize].start(mine_job).await;
            }
        });
        let send_h = tokio::task::spawn({
            let rpc = rpc.clone();
            async move {
                start_sender(rpc, tip_amount, send_rx, tunnel_tx, kp_filepath).await;
            }
        });
        Foreman {
            rpc: rpc.clone(),
            tunnels,
            mine_tx,
            tunnel_rx,
            _mine_h: mine_h,
            _send_h: send_h,
        }
    }

    pub async fn start(&mut self) {
        // Start loop to listen to treasury
        let treasury = Arc::new(RwLock::new(fetch_treasury(self.rpc.clone()).await));
        self.subscribe_treasury(treasury.clone());

        // TODO Register all proofs if necessary
        for tunnel in self.tunnels.iter() {
            // TODO
        }

        // TODO Start loop
        while let Some(tunnel) = self.tunnel_rx.recv().await {
            println!("Got tunnel: {:?}", tunnel);
            tokio::task::spawn({
                let treasury = treasury.clone();
                let mine_tx = self.mine_tx.clone();
                let rpc = self.rpc.clone();
                async move {
                    // TODO Top up keypair if necessary
                    // TODO Fetch challenge
                    let proof = fetch_or_register(rpc, tunnel.proof_address).await;
                    let r_treasury = treasury.read().await;
                    let mine_job = MineJob {
                        tunnel_id: tunnel.id,
                        difficulty: r_treasury.difficulty.into(),
                        challenge: proof.hash.into(),
                    };
                    mine_tx.send(mine_job).ok();
                }
            });
        }
    }

    pub fn subscribe_treasury(&self, treasury: Arc<RwLock<Treasury>>) {
        tokio::spawn({
            let treasury = treasury.clone();
            let rpc = self.rpc.clone();
            async move {
                let mut interval = time::interval(Duration::from_secs(30));
                loop {
                    interval.tick().await;
                    let new_treasury = fetch_treasury(rpc.clone()).await;
                    let mut w_tresaury = treasury.write().await;
                    *w_tresaury = new_treasury;
                }
            }
        });
    }
}

#[derive(Debug)]
struct Tunnel {
    // TODO Cached proof account state
    // TODO Keypair
    pub id: u64,
    pub pubkey: Pubkey,
    pub proof_address: Pubkey,
    pub send_tx: UnboundedSender<SendJob>,
}

impl Tunnel {
    pub fn new(id: u64, send_tx: UnboundedSender<SendJob>) -> Tunnel {
        // Get or create keypair for tunnel
        let kp_path = tunnel_keypair_filepath(id).expect("Failed to get keypair filepath");
        let kp = match read_keypair_file(kp_path.clone()) {
            Ok(kp) => kp,
            Err(_) => {
                let kp = Keypair::new();
                write_keypair_file(&kp, kp_path).expect("Failed to create keypair file");
                kp
            }
        };

        // Build the tunnel
        Tunnel {
            id,
            pubkey: kp.pubkey(),
            proof_address: proof_address(kp.pubkey()),
            send_tx,
        }
    }

    pub async fn start(&self, job: MineJob) {
        println!("hashing...");
        let mut next_hash: Hash;
        for nonce in 1_u64.. {
            next_hash = hashv(&[
                job.challenge.to_bytes().as_slice(),
                self.pubkey.to_bytes().as_slice(),
                nonce.to_le_bytes().as_slice(),
            ]);
            if next_hash.le(&job.difficulty) {
                let send_job = SendJob {
                    proof: self.proof_address,
                    signer: self.pubkey,
                    hash: next_hash,
                    nonce,
                    new: false,
                };
                self.send_tx.send(send_job).ok();
                return;
            }
        }
    }
}

struct MineJob {
    tunnel_id: u64,
    challenge: Hash,
    difficulty: Hash,
}

// TODO Jito rpc
async fn start_sender(
    rpc: Arc<RpcClient>,
    tip_amount: u64,
    mut send_rx: UnboundedReceiver<SendJob>,
    tunnel_tx: UnboundedSender<Arc<Tunnel>>,
    kp_filepath: String,
) {
    tokio::task::spawn(async move {
        loop {
            // TODO Latest blockhash
            while let Some(send_job) = send_rx.recv().await {
                println!("Got job: {:?}", send_job);
                // TODO
            }
        }
    });
    // TODO Fetch the keypair or panic
    // TODO Start loop to fetch latest blockhash
    // TODO listen on channel
}

#[derive(Debug)]
struct SendJob {
    proof: Pubkey,
    signer: Pubkey,
    hash: Hash,
    nonce: u64,
    new: bool, // Should add ix to register proof account
}

async fn fetch_or_register(rpc: Arc<RpcClient>, address: Pubkey) -> Proof {
    if let Some(proof) = fetch_proof(rpc.clone(), address).await {
        proof
    } else {
        // TODO register
        Proof {
            authority: address,              // todo!(),
            claimable_rewards: 0,            // todo!(),
            hash: Hash::new_unique().into(), // todo!(),
            total_hashes: 0,                 // todo!(),
            total_rewards: 0,                // todo!(),
        }
    }
}

async fn fetch_proof(rpc: Arc<RpcClient>, address: Pubkey) -> Option<Proof> {
    let mut attempts = 0;
    loop {
        match rpc.get_account_data(&address).await {
            Ok(data) => {
                return Some(*Proof::try_from_bytes(&data).expect("Failed to parse proof account"));
            }
            Err(err) => {
                println!("Error: {:?}", err);
                if let solana_client::client_error::ClientErrorKind::RpcError(rpc_err) = err.kind {
                    if let solana_client::rpc_request::RpcError::ForUser(msg) = rpc_err {
                        if msg.contains("AccountNotFound") {
                            return None;
                        }
                    }
                }
            }
        }
        attempts += 1;
        if attempts > 4 {
            return None;
        }
    }
}

async fn fetch_treasury(rpc: Arc<RpcClient>) -> Treasury {
    // TODO Retries
    let data = rpc
        .get_account_data(&TREASURY_ADDRESS)
        .await
        .expect("Failed to fetch treasury");
    *Treasury::try_from_bytes(&data).expect("Failed to parse treasury account")
}

pub fn tunnel_keypair_filepath(id: u64) -> io::Result<PathBuf> {
    let home_dir = dirs::home_dir().expect("Home directory not found.");
    let ore_path = home_dir.join(".config").join("ore");
    fs::create_dir_all(&ore_path)?;
    Ok(ore_path.join(format!("tunnel-{}.json", id)))
}

#[cached]
pub fn proof_address(authority: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[PROOF, authority.as_ref()], &ore::ID).0
}

// TODO JitoClient
