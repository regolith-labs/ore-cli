mod blockhash;
mod foreman;
mod messages;
mod miner;
mod sender;
mod treasury;
mod utils;

use clap::{command, Parser, Subcommand};
use foreman::Foreman;
use logfather::{Level, Logger};

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
    #[command(about = "Mine ore using your local cpu")]
    Mine(MineArgs),
}

#[derive(Parser, Debug)]
struct MineArgs {
    #[arg(
        long,
        value_name = "TUNNELS",
        help = "The number of threads to dedicate to mining",
        default_value = "1"
    )]
    tunnels: u64,

    #[arg(
        long,
        value_name = "LAMPORTS",
        help = "Number of lamports to pay as tip in each jito bundle",
        default_value = "0"
    )]
    pub tip: u64,
}

#[tokio::main]
async fn main() {
    // Parse args
    let args = Args::parse();

    // Setup logger
    let mut logger = Logger::new();
    logger.level(Level::Trace);
    logger.path("ore.log");
    logger.file(true);

    // Execute command
    match args.command {
        Commands::Mine(mine_args) => {
            Foreman::start(
                args.keypair.expect("expected keypair"),
                args.rpc.expect("expected rpc"),
                args.priority_fee,
                mine_args.tip,
                mine_args.tunnels,
            )
            .await;
        }
    }
}
