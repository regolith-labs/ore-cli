mod config;
mod init;
mod address;
mod balance;
mod send;
mod lookup_table;
mod stake;
mod tx;
mod keypair;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "ore", about = "ORE smart wallet with post-quantum security")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// View or set persistent configuration
    Config {
        #[command(subcommand)]
        action: Option<ConfigAction>,
    },
    /// Initialize a new smart wallet
    Init,
    /// Print smart wallet address
    Address,
    /// Show ORE token balance
    Balance,
    /// Transfer ORE to a recipient
    Transfer {
        /// Amount of ORE to transfer (e.g., "1.5")
        amount: String,
        /// Recipient wallet address
        recipient: String,
    },
    /// Manage ORE stake
    Stake {
        #[command(subcommand)]
        action: StakeAction,
    },
}

#[derive(Subcommand)]
enum StakeAction {
    /// Deposit ORE into stake
    Deposit {
        /// Amount of ORE to deposit (e.g., "1.5")
        amount: String,
    },
    /// Withdraw ORE from stake
    Withdraw {
        /// Amount of ORE to withdraw (e.g., "1.5")
        amount: String,
    },
    /// Show staked balance and yield
    Balance,
    /// Claim all yield
    Claim,
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Set a config value
    Set {
        /// Key to set (keypair, fee-payer, rpc-url)
        key: String,
        /// Value to set
        value: String,
    },
}

fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Commands::Config { action } => config::run(action),
        Commands::Init => init::run(),
        Commands::Address => address::run(),
        Commands::Balance => balance::run(),
        Commands::Transfer { amount, recipient } => send::run(&amount, &recipient),
        Commands::Stake { action } => match action {
            StakeAction::Deposit { amount } => stake::run_deposit(&amount),
            StakeAction::Withdraw { amount } => stake::run_withdraw(&amount),
            StakeAction::Balance => stake::run_balance(),
            StakeAction::Claim => stake::run_claim(),
        },
    };
    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
