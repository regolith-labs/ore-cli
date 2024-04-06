use std::error::Error;
use cached::proc_macro::cached;
use ore::{
    self,
    MINT_ADDRESS,
    PROOF,
    state::{Proof, Treasury}, TREASURY_ADDRESS, utils::AccountDeserialize,
};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_program::{pubkey::Pubkey, sysvar};
use solana_sdk::{clock::Clock, commitment_config::CommitmentConfig};
use spl_associated_token_account::get_associated_token_address;

pub async fn get_treasury(cluster: String) -> Result<Treasury, Box<dyn Error>> {
    let client = RpcClient::new_with_commitment(cluster, CommitmentConfig::finalized());
    let data = client
        .get_account_data(&TREASURY_ADDRESS)
        .await?;
    Ok(*Treasury::try_from_bytes(&data)?)
}

pub async fn get_proof(cluster: String, authority: Pubkey) -> Result<Proof, Box<dyn Error>> {
    let client = RpcClient::new_with_commitment(cluster, CommitmentConfig::finalized());
    let proof_address = proof_pubkey(authority);
    let data = client
        .get_account_data(&proof_address)
        .await?;
    Ok(*Proof::try_from_bytes(&data)?)
}

pub async fn get_clock_account(cluster: String) -> Result<Clock, Box<dyn Error>> {
    let client = RpcClient::new_with_commitment(cluster, CommitmentConfig::finalized());
    let data = client
        .get_account_data(&sysvar::clock::ID)
        .await?;
    // bincode::deserialize::<Clock>(&data).map_err(Into::into)
    Ok(bincode::deserialize::<Clock>(&data)?)
}



#[cached]
pub fn proof_pubkey(authority: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[PROOF, authority.as_ref()], &ore::ID).0
}

#[cached]
pub fn treasury_tokens_pubkey() -> Pubkey {
    get_associated_token_address(&TREASURY_ADDRESS, &MINT_ADDRESS)
}
