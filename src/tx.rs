use indicatif::{ProgressBar, ProgressStyle};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{signature::Signature, transaction::{Transaction, VersionedTransaction}};

fn make_spinner(msg: &str) -> ProgressBar {
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
            .template("{spinner:.cyan} {msg}")
            .unwrap(),
    );
    spinner.set_message(msg.to_string());
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));
    spinner
}

fn finish_spinner(spinner: &ProgressBar, result: &Result<Signature, impl std::fmt::Display>) {
    match result {
        Ok(sig) => {
            spinner.finish_and_clear();
            println!("Confirmed: {sig}");
        }
        Err(e) => {
            spinner.finish_and_clear();
            eprintln!("Failed: {e}");
        }
    }
}

/// Send a legacy transaction with a spinner.
pub fn send_and_confirm(rpc: &RpcClient, tx: &Transaction, msg: &str) -> anyhow::Result<Signature> {
    let spinner = make_spinner(msg);
    let result = rpc.send_and_confirm_transaction(tx);
    finish_spinner(&spinner, &result);
    Ok(result?)
}

/// Send a versioned (v0) transaction with a spinner.
pub fn send_and_confirm_versioned(rpc: &RpcClient, tx: &VersionedTransaction, msg: &str) -> anyhow::Result<Signature> {
    let spinner = make_spinner(msg);
    let result = rpc.send_and_confirm_transaction(tx);
    finish_spinner(&spinner, &result);
    Ok(result?)
}
