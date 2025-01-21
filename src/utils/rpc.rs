use std::time::Duration;

use ore_api::{
    consts::{
        CONFIG_ADDRESS, TREASURY_ADDRESS,
    },
    state::{proof_pda, Bus, Config, Proof, Treasury},
};
use ore_boost_api::state::{Boost, Stake, Reservation};
use ore_pool_api::state::{Pool, Member, Share};
use serde::Deserialize;
use solana_account_decoder::UiAccountEncoding;
use solana_client::{client_error::{ClientError, ClientErrorKind}, rpc_filter::{RpcFilterType, Memcmp}, rpc_config::{RpcProgramAccountsConfig, RpcAccountInfoConfig}};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_program::{pubkey::Pubkey, sysvar, program_pack::Pack};
use solana_sdk::{clock::Clock, hash::Hash};
use spl_token::state::Mint;
use steel::{AccountDeserialize, Discriminator};
use tokio::time::sleep;

#[cfg(feature = "admin")]
use ore_boost_api::state::Checkpoint;

pub const BLOCKHASH_QUERY_RETRIES: usize = 5;
pub const BLOCKHASH_QUERY_DELAY: u64 = 500;

pub enum ComputeBudget {
    #[allow(dead_code)]
    Dynamic,
    Fixed(u32),
}

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
                account_config: RpcAccountInfoConfig {
                    encoding: Some(UiAccountEncoding::Base64),
                    ..Default::default()
                },
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

pub async fn get_mint(client: &RpcClient, address: Pubkey) -> Result<Mint, anyhow::Error> {
    let mint_data = client.get_account_data(&address).await?;
    let mint = Mint::unpack(&mint_data)?;
    Ok(mint)
}

pub async fn get_config(client: &RpcClient) -> Config {
    let data = client
        .get_account_data(&CONFIG_ADDRESS)
        .await
        .expect("Failed to get config account");
    *Config::try_from_bytes(&data).expect("Failed to parse config account")
}

pub async fn get_boost(client: &RpcClient, address: Pubkey) -> Result<Boost, anyhow::Error> {
    let data = client
        .get_account_data(&address)
        .await?;
    Ok(*Boost::try_from_bytes(&data).expect("Failed to parse boost account"))
}

pub async fn get_boosts(client: &RpcClient) -> Result<Vec<(Pubkey, Boost)>, anyhow::Error> {
    get_program_accounts::<Boost>(client, ore_boost_api::ID, vec![]).await
}

pub async fn get_pools(client: &RpcClient) -> Result<Vec<(Pubkey, Pool)>, anyhow::Error> {
    get_program_accounts::<Pool>(client, ore_pool_api::ID, vec![]).await
}


#[cfg(feature = "admin")]
pub async fn get_checkpoint(client: &RpcClient, address: Pubkey) -> Checkpoint {
    let data = client
        .get_account_data(&address)
        .await
        .expect("Failed to get checkpoint account");
    *Checkpoint::try_from_bytes(&data).expect("Failed to parse checkpoint account")
}

pub async fn get_pool(client: &RpcClient, address: Pubkey) -> Result<Pool, anyhow::Error> {
    let data = client
        .get_account_data(&address)
        .await?;
    Ok(*Pool::try_from_bytes(&data)?)
}

pub async fn get_member(client: &RpcClient, address: Pubkey) -> Result<Member, anyhow::Error> {
    let data = client
        .get_account_data(&address)
        .await?;
    Ok(*Member::try_from_bytes(&data)?)
}

pub async fn get_stake(client: &RpcClient, address: Pubkey) -> Result<Stake, anyhow::Error> {
    let data = client
        .get_account_data(&address)
        .await?;
    Ok(*Stake::try_from_bytes(&data)?)
}

pub async fn get_share(client: &RpcClient, address: Pubkey) -> Result<Share, anyhow::Error> {
    let data = client
        .get_account_data(&address)
        .await?;
    Ok(*Share::try_from_bytes(&data)?)
}

pub async fn get_bus(client: &RpcClient, address: Pubkey) -> Bus {
    let data = client
        .get_account_data(&address)
        .await
        .expect("Failed to get stake account");
    *Bus::try_from_bytes(&data).expect("Failed to parse bus account")
}

pub async fn get_legacy_stake(client: &RpcClient, address: Pubkey) -> Result<ore_boost_legacy_api::state::Stake, anyhow::Error> {
    let data = client
        .get_account_data(&address)
        .await?;
    Ok(*ore_boost_legacy_api::state::Stake::try_from_bytes(&data)?)
}

pub async fn get_boost_stake_accounts(
    rpc_client: &RpcClient,
    boost_address: Pubkey,
) -> Result<Vec<(Pubkey, Stake)>, anyhow::Error> {
    let filter =  RpcFilterType::Memcmp(Memcmp::new_raw_bytes(
        56,
        boost_address.to_bytes().to_vec(),
    ));
    get_program_accounts::<Stake>(rpc_client, ore_boost_api::ID, vec![filter]).await
}

pub async fn get_proof_with_authority(client: &RpcClient, authority: Pubkey) -> Result<Proof, anyhow::Error> {
    let proof_address = proof_pda(authority).0;
    get_proof(client, proof_address).await
}

pub async fn get_updated_proof_with_authority(
    client: &RpcClient,
    authority: Pubkey,
    lash_hash_at: i64,
) -> Result<Proof, anyhow::Error> {
    loop {
        if let Ok(proof) = get_proof_with_authority(client, authority).await {
            if proof.last_hash_at.gt(&lash_hash_at) {
                return Ok(proof);
            }
        }
        tokio::time::sleep(Duration::from_millis(1_000)).await;
    }
}

pub async fn get_proof(client: &RpcClient, address: Pubkey) -> Result<Proof, anyhow::Error> {
    let data = client
        .get_account_data(&address)
        .await?;
    Ok(*Proof::try_from_bytes(&data)?)
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
