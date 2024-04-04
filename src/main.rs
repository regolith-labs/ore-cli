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

use std::sync::Arc;

use clap::{command, Parser, Subcommand};
use solana_client::{
    client_error::reqwest::Url, connection_cache::ConnectionCache,
    nonblocking::rpc_client::RpcClient,
};
use solana_sdk::{
    commitment_config::CommitmentConfig,
    signature::{read_keypair_file, Keypair},
};

struct Miner {
    pub(crate) keypair_filepath: Option<String>,
    pub(crate) priority_fee: u64,
    pub(crate) connection_cache: ConnectionCache,
    pub(crate) rpc_client: Arc<RpcClient>,
    pub(crate) websocket_url: String,
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
        value_name = "KEYPAIR_FILEPATH",
        help = "Filepath to keypair to use"
    )]
    keypair: Option<String>,

    #[arg(
        long,
        value_name = "MICROLAMPORTS",
        help = "Number of microlamports to pay as priority fee per transaction",
        default_value = "100000"
    )]
    priority_fee: u64,

    #[arg(
        long,
        value_name = "USE_QUIC",
        help = "Use quic or udp",
        default_value = "true"
    )]
    use_quic: bool,

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
    let args = Args::parse();

    let websocket_url = compute_websocket_url(&args.rpc);

    let connection_cache = if args.use_quic {
        ConnectionCache::new_quic("connection_cache_ore_cli_quic", 1)
    } else {
        ConnectionCache::with_udp("connection_cache_ore_cli_udp", 1)
    };

    let rpc_client = Arc::new(RpcClient::new_with_commitment(
        args.rpc,
        CommitmentConfig::processed(),
    ));

    // Initialize miner.
    let miner = Arc::new(Miner::new(
        args.keypair,
        args.priority_fee,
        rpc_client,
        connection_cache,
        websocket_url,
    ));

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
            miner.claim(args.beneficiary, args.amount).await;
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
    pub fn new(
        keypair_filepath: Option<String>,
        priority_fee: u64,
        rpc_client: Arc<RpcClient>,
        connection_cache: ConnectionCache,
        websocket_url: String,
    ) -> Self {
        Self {
            keypair_filepath,
            priority_fee,
            rpc_client,
            connection_cache,
            websocket_url,
        }
    }

    pub fn signer(&self) -> Keypair {
        match self.keypair_filepath.clone() {
            Some(filepath) => read_keypair_file(filepath).unwrap(),
            None => panic!("No keypair provided"),
        }
    }
}

fn compute_websocket_url(json_rpc_url: &str) -> String {
    let json_rpc_url: Option<Url> = json_rpc_url.parse().ok();
    if json_rpc_url.is_none() {
        return "".to_string();
    }
    let json_rpc_url = json_rpc_url.unwrap();
    let is_secure = json_rpc_url.scheme().to_ascii_lowercase() == "https";
    let mut ws_url = json_rpc_url.clone();
    ws_url
        .set_scheme(if is_secure { "wss" } else { "ws" })
        .expect("unable to set scheme");
    if let Some(port) = json_rpc_url.port() {
        let port = port.checked_add(1).expect("port out of range");
        ws_url.set_port(Some(port)).expect("unable to set port");
    }
    ws_url.to_string()
}
