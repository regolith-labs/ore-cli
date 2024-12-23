use std::{io::Read, time::Duration};

use cached::proc_macro::cached;
use ore_api::{
    consts::{
        CONFIG_ADDRESS, MINT_ADDRESS, PROOF, TOKEN_DECIMALS, TREASURY_ADDRESS,
    },
    state::{Config, Proof, Treasury},
};
use ore_boost_api::state::{Boost, Stake, Checkpoint, Reservation};
use serde::Deserialize;
use solana_client::{client_error::{ClientError, ClientErrorKind}, rpc_filter::{RpcFilterType, Memcmp}, rpc_config::RpcProgramAccountsConfig};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_program::{pubkey::Pubkey, sysvar};
use solana_sdk::{clock::Clock, hash::Hash};
use spl_associated_token_account::get_associated_token_address;
use steel::{AccountDeserialize, Discriminator};
use tokio::time::sleep;

pub const BLOCKHASH_QUERY_RETRIES: usize = 5;
pub const BLOCKHASH_QUERY_DELAY: u64 = 500;

pub async fn get_program_accounts<T>(client: &RpcClient, program_id: Pubkey, filters: Vec<RpcFilterType>) -> Result<Vec<(Pubkey, T)>, anyhow::Error> 
    where T: AccountDeserialize + Discriminator + Clone {
    let mut all_filters = vec![
        RpcFilterType::Memcmp(Memcmp::new_raw_bytes(
            0,
            T::discriminator().to_le_bytes().to_vec(),
        )),
    ];
    all_filters.extend(filters);
    let accounts = client
        .get_program_accounts_with_config(
            &program_id,
            RpcProgramAccountsConfig {
                filters: Some(all_filters),
                ..Default::default()
            },
        )
        .await?
        .into_iter()
        .map(|(pubkey, account)| {
            let account = T::try_from_bytes(&account.data).unwrap().clone();
            (pubkey, account)
        })
        .collect();

    Ok(accounts)
}

pub async fn _get_treasury(client: &RpcClient) -> Treasury {
    let data = client
        .get_account_data(&TREASURY_ADDRESS)
        .await
        .expect("Failed to get treasury account");
    *Treasury::try_from_bytes(&data).expect("Failed to parse treasury account")
}

pub async fn get_config(client: &RpcClient) -> Config {
    let data = client
        .get_account_data(&CONFIG_ADDRESS)
        .await
        .expect("Failed to get config account");
    *Config::try_from_bytes(&data).expect("Failed to parse config account")
}

pub async fn get_boost(client: &RpcClient, address: Pubkey) -> Boost {
    let data = client
        .get_account_data(&address)
        .await
        .expect("Failed to get boost account");
    *Boost::try_from_bytes(&data).expect("Failed to parse boost account")
}

pub async fn _get_boosts(client: &RpcClient, reserved_for: Option<Pubkey>) -> Result<Vec<(Pubkey, Boost)>, anyhow::Error> {
    let mut filters = vec![];
    if let Some(reserved_for) = reserved_for {
        filters.push(RpcFilterType::Memcmp(Memcmp::new_raw_bytes(
            72,
            reserved_for.to_bytes().to_vec(),
        )));
    }
    get_program_accounts::<Boost>(client, ore_boost_api::ID, filters).await
}

pub async fn get_checkpoint(client: &RpcClient, address: Pubkey) -> Checkpoint {
    let data = client
        .get_account_data(&address)
        .await
        .expect("Failed to get checkpoint account");
    *Checkpoint::try_from_bytes(&data).expect("Failed to parse checkpoint account")
}

pub async fn get_stake(client: &RpcClient, address: Pubkey) -> Stake {
    let data = client
        .get_account_data(&address)
        .await
        .expect("Failed to get stake account");
    *Stake::try_from_bytes(&data).expect("Failed to parse stake account")
}

pub async fn get_legacy_stake(client: &RpcClient, address: Pubkey) -> ore_boost_legacy_api::state::Stake {
    let data = client
        .get_account_data(&address)
        .await
        .expect("Failed to get stake account");
    *ore_boost_legacy_api::state::Stake::try_from_bytes(&data).expect("Failed to parse stake account")
}

pub async fn get_stake_accounts(
    rpc_client: &RpcClient,
    boost_address: Pubkey,
) -> Result<Vec<(Pubkey, Stake)>, anyhow::Error> {
    let filter =  RpcFilterType::Memcmp(Memcmp::new_raw_bytes(
        48,
        boost_address.to_bytes().to_vec(),
    ));
    get_program_accounts::<Stake>(rpc_client, ore_boost_api::ID, vec![filter]).await
}

pub async fn get_proof_with_authority(client: &RpcClient, authority: Pubkey) -> Proof {
    let proof_address = proof_pubkey(authority);
    get_proof(client, proof_address).await
}

pub async fn get_updated_proof_with_authority(
    client: &RpcClient,
    authority: Pubkey,
    lash_hash_at: i64,
) -> Proof {
    loop {
        let proof = get_proof_with_authority(client, authority).await;
        if proof.last_hash_at.gt(&lash_hash_at) {
            return proof;
        }
        tokio::time::sleep(Duration::from_millis(1_000)).await;
    }
}

pub async fn get_proof(client: &RpcClient, address: Pubkey) -> Proof {
    let data = client
        .get_account_data(&address)
        .await
        .expect("Failed to get proof account");
    *Proof::try_from_bytes(&data).expect("Failed to parse proof account")
}

pub async fn get_reservation(client: &RpcClient, address: Pubkey) -> Result<Reservation, anyhow::Error> {
    let data = client
        .get_account_data(&address)
        .await?;
    let reservation = Reservation::try_from_bytes(&data)?;
    Ok(*reservation)
}


pub async fn get_clock(client: &RpcClient) -> Clock {
    let data = client
        .get_account_data(&sysvar::clock::ID)
        .await
        .expect("Failed to get clock account");
    bincode::deserialize::<Clock>(&data).expect("Failed to deserialize clock")
}

pub fn amount_u64_to_string(amount: u64) -> String {
    amount_u64_to_f64(amount).to_string()
}

pub fn amount_u64_to_f64(amount: u64) -> f64 {
    (amount as f64) / 10f64.powf(TOKEN_DECIMALS as f64)
}

pub fn amount_f64_to_u64(amount: f64) -> u64 {
    (amount * 10f64.powf(TOKEN_DECIMALS as f64)) as u64
}

pub fn ask_confirm(question: &str) -> bool {
    println!("{}", question);
    loop {
        let mut input = [0];
        let _ = std::io::stdin().read(&mut input);
        match input[0] as char {
            'y' | 'Y' => return true,
            'n' | 'N' => return false,
            _ => println!("y/n only please."),
        }
    }
}

pub async fn get_latest_blockhash_with_retries(
    client: &RpcClient,
) -> Result<(Hash, u64), ClientError> {
    let mut attempts = 0;

    loop {
        if let Ok((hash, slot)) = client
            .get_latest_blockhash_with_commitment(client.commitment())
            .await
        {
            return Ok((hash, slot));
        }

        // Retry
        sleep(Duration::from_millis(BLOCKHASH_QUERY_DELAY)).await;
        attempts += 1;
        if attempts >= BLOCKHASH_QUERY_RETRIES {
            return Err(ClientError {
                request: None,
                kind: ClientErrorKind::Custom(
                    "Max retries reached for latest blockhash query".into(),
                ),
            });
        }
    }
}

#[cached]
pub fn proof_pubkey(authority: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[PROOF, authority.as_ref()], &ore_api::ID).0
}

#[cached]
pub fn treasury_tokens_pubkey() -> Pubkey {
    get_associated_token_address(&TREASURY_ADDRESS, &MINT_ADDRESS)
}

#[derive(Debug, Deserialize)]
pub struct Tip {
    pub time: String,
    pub landed_tips_25th_percentile: f64,
    pub landed_tips_50th_percentile: f64,
    pub landed_tips_75th_percentile: f64,
    pub landed_tips_95th_percentile: f64,
    pub landed_tips_99th_percentile: f64,
    pub ema_landed_tips_50th_percentile: f64,
}
