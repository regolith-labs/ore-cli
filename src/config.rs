use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::ConfigAction;

/// Persistent config stored at ~/.config/ore/config.toml
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct OreConfig {
    /// Path to the Falcon-512 keypair file
    pub keypair: Option<String>,
    /// Path to the Solana fee-payer keypair
    pub fee_payer: Option<String>,
    /// RPC URL
    pub rpc_url: Option<String>,
    /// Address Lookup Table (created by `ore init`)
    pub lookup_table: Option<String>,
}

impl OreConfig {
    pub fn config_path() -> PathBuf {
        let home = dirs::home_dir().expect("cannot determine home directory");
        home.join(".config").join("ore").join("config.toml")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            let contents = fs::read_to_string(&path).unwrap_or_default();
            toml::from_str(&contents).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let contents = toml::to_string_pretty(self)?;
        fs::write(&path, contents)?;
        Ok(())
    }

    /// Resolve the Falcon-512 keypair path.
    /// Default: ~/.config/solana/ore-falcon512.bin
    pub fn keypair_path(&self) -> PathBuf {
        if let Some(ref p) = self.keypair {
            expand_tilde(p)
        } else {
            default_falcon_keypair_path()
        }
    }

    /// Resolve the fee-payer keypair path.
    /// Falls back to the Solana CLI configured keypair.
    pub fn fee_payer_path(&self) -> PathBuf {
        if let Some(ref p) = self.fee_payer {
            expand_tilde(p)
        } else {
            solana_cli_keypair_path()
        }
    }

    /// Resolve the RPC URL.
    /// Falls back to the Solana CLI configured RPC URL.
    pub fn rpc_url(&self) -> String {
        if let Some(ref url) = self.rpc_url {
            url.clone()
        } else {
            solana_cli_rpc_url()
        }
    }
}

fn default_falcon_keypair_path() -> PathBuf {
    let home = dirs::home_dir().expect("cannot determine home directory");
    home.join(".config").join("solana").join("falcon512.bin")
}

fn solana_cli_config_path() -> PathBuf {
    let home = dirs::home_dir().expect("cannot determine home directory");
    home.join(".config")
        .join("solana")
        .join("cli")
        .join("config.yml")
}

fn solana_cli_keypair_path() -> PathBuf {
    if let Ok(contents) = fs::read_to_string(solana_cli_config_path()) {
        for line in contents.lines() {
            if let Some(rest) = line.strip_prefix("keypair_path:") {
                let val = rest.trim().trim_matches('"').trim_matches('\'');
                return expand_tilde(val);
            }
        }
    }
    // Fallback
    let home = dirs::home_dir().expect("cannot determine home directory");
    home.join(".config").join("solana").join("id.json")
}

fn solana_cli_rpc_url() -> String {
    if let Ok(contents) = fs::read_to_string(solana_cli_config_path()) {
        for line in contents.lines() {
            if let Some(rest) = line.strip_prefix("json_rpc_url:") {
                return rest.trim().trim_matches('"').trim_matches('\'').to_string();
            }
        }
    }
    "https://api.mainnet-beta.solana.com".to_string()
}

fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        let home = dirs::home_dir().expect("cannot determine home directory");
        home.join(rest)
    } else {
        PathBuf::from(path)
    }
}

pub fn run(action: Option<ConfigAction>) -> anyhow::Result<()> {
    let mut cfg = OreConfig::load();

    match action {
        None => {
            // Print current effective config
            println!("Config file: {}", OreConfig::config_path().display());
            println!();
            println!("  keypair:   {}", cfg.keypair_path().display());
            println!("  fee-payer: {}", cfg.fee_payer_path().display());
            println!("  rpc-url:   {}", cfg.rpc_url());
        }
        Some(ConfigAction::Set { key, value }) => {
            match key.as_str() {
                "keypair" => cfg.keypair = Some(value),
                "fee-payer" => cfg.fee_payer = Some(value),
                "rpc-url" => cfg.rpc_url = Some(value),
                _ => anyhow::bail!("unknown config key: {key} (valid: keypair, fee-payer, rpc-url)"),
            }
            cfg.save()?;
            println!("Config updated.");
        }
    }

    Ok(())
}
