use std::sync::Arc;

use clap::{command, Parser, Subcommand};
use solana_sdk::signature::Keypair;

mod balance;
mod busses;
mod claim;
#[cfg(feature = "admin")]
mod initialize;
mod mine;
mod register;
mod rewards;
mod send_and_confirm;
mod treasury;
#[cfg(feature = "admin")]
mod update_admin;
#[cfg(feature = "admin")]
mod update_difficulty;
mod utils;

struct Miner {
    pub keypair_private_key: Option<String>,
    pub priority_fee: u64,
    pub cluster: String,
    pub send_tx_cluster: String,
}

#[derive(Parser, Debug)]
#[command(about, version)]
struct Args {
    #[arg(
    long,
    value_name = "NETWORK_URL",
    help = "Network address of your RPC provider",
    default_value = "https://api.mainnet-beta.solana.com"
    )]
    rpc: String,

    #[arg(
    long,
    value_name = "SEND_TX_RPC",
    help = "Network address of your RPC provider for send transactions",
    default_value = ""
    )]
    send_tx_rpc: String,

    #[arg(
    long,
    value_name = "keypair_private_key",
    help = "Filepath to keypair to use"
    )]
    keypair: Option<String>,

    #[arg(
    long,
    value_name = "MICROLAMPORTS",
    help = "Number of microlamports to pay as priority fee per transaction",
    default_value = "0"
    )]
    priority_fee: u64,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    #[command(about = "Fetch the Ore balance of an account")]
    Balance(BalanceArgs),

    #[command(about = "Fetch the distributable rewards of the busses")]
    Busses(BussesArgs),

    #[command(about = "Mine Ore using local compute")]
    Mine(MineArgs),

    #[command(about = "Claim available mining rewards")]
    Claim(ClaimArgs),

    #[command(about = "Fetch your balance of unclaimed mining rewards")]
    Rewards(RewardsArgs),

    #[command(about = "Fetch the treasury account and balance")]
    Treasury(TreasuryArgs),

    #[cfg(feature = "admin")]
    #[command(about = "Initialize the program")]
    Initialize(InitializeArgs),

    #[cfg(feature = "admin")]
    #[command(about = "Update the program admin authority")]
    UpdateAdmin(UpdateAdminArgs),

    #[cfg(feature = "admin")]
    #[command(about = "Update the mining difficulty")]
    UpdateDifficulty(UpdateDifficultyArgs),
}

#[derive(Parser, Debug)]
struct BalanceArgs {
    #[arg(
    // long,
    value_name = "ADDRESS",
    help = "The address of the account to fetch the balance of"
    )]
    pub address: Option<String>,
}

#[derive(Parser, Debug)]
struct BussesArgs {}

#[derive(Parser, Debug)]
struct RewardsArgs {
    #[arg(
    // long,
    value_name = "ADDRESS",
    help = "The address of the account to fetch the rewards balance of"
    )]
    pub address: Option<String>,
}

#[derive(Parser, Debug)]
struct MineArgs {
    #[arg(
    long,
    short,
    value_name = "THREAD_COUNT",
    help = "The number of threads to dedicate to mining",
    default_value = "1"
    )]
    threads: u64,
}

#[derive(Parser, Debug)]
struct TreasuryArgs {}

#[derive(Parser, Debug)]
struct ClaimArgs {
    #[arg(
    // long,
    value_name = "AMOUNT",
    help = "The amount of rewards to claim. Defaults to max."
    )]
    amount: Option<f64>,

    #[arg(
    // long,
    value_name = "TOKEN_ACCOUNT_ADDRESS",
    help = "Token account to receive mining rewards."
    )]
    beneficiary: Option<String>,

    #[arg(
    // long,
    value_name = "RETRY_COUNT",
    help = "Send transaction retry count. Defaults to 100",
    default_value = "100"
    )]
    retry_count: u64,
}

#[cfg(feature = "admin")]
#[derive(Parser, Debug)]
struct InitializeArgs {}

#[cfg(feature = "admin")]
#[derive(Parser, Debug)]
struct UpdateAdminArgs {
    new_admin: String,
}

#[cfg(feature = "admin")]
#[derive(Parser, Debug)]
struct UpdateDifficultyArgs {}

#[tokio::main]
async fn main() {
    // Initialize miner.
    let mut args = Args::parse();
    if args.send_tx_rpc.is_empty() {
        args.send_tx_rpc = args.rpc.clone();
    }
    println!("rpc: {}", args.rpc.clone());
    println!("send_rpc: {}", args.send_tx_rpc.clone());
    let cluster = args.rpc;

    let miner = Arc::new(Miner::new(cluster.clone(), args.send_tx_rpc.clone(), args.priority_fee, args.keypair));

    // Execute user command.
    match args.command {
        Commands::Balance(args) => {
            miner.balance(args.address).await;
        }
        Commands::Busses(_) => {
            miner.busses().await;
        }
        Commands::Rewards(args) => {
            miner.rewards(args.address).await;
        }
        Commands::Treasury(_) => {
            miner.treasury().await;
        }
        Commands::Mine(args) => {
            miner.mine(args.threads).await;
        }
        Commands::Claim(args) => {
            miner.claim(cluster, args.beneficiary, args.amount, args.retry_count).await;
        }
        #[cfg(feature = "admin")]
        Commands::Initialize(_) => {
            miner.initialize().await;
        }
        #[cfg(feature = "admin")]
        Commands::UpdateAdmin(args) => {
            miner.update_admin(args.new_admin).await;
        }
        #[cfg(feature = "admin")]
        Commands::UpdateDifficulty(_) => {
            miner.update_difficulty().await;
        }
    }
}

impl Miner {
    pub fn new(cluster: String, send_tx_cluster: String, priority_fee: u64, keypair_private_key: Option<String>) -> Self {
        Self {
            keypair_private_key,
            priority_fee,
            cluster,
            send_tx_cluster,
        }
    }

    pub fn signer(&self) -> Keypair {
        match self.keypair_private_key.clone() {
            Some(key) => Keypair::from_base58_string(&key),
            None => panic!("No keypair provided"),
        }
    }
}
