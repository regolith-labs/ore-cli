use std::str::FromStr;
use solana_program::pubkey::Pubkey;
use solana_sdk::signature::Signer;

use crate::{
    args::RankArgs,
    send_and_confirm::ComputeBudget,
    utils::{proof_pubkey, get_proof},
    Miner,
};

impl Miner {
    pub async fn rank(&self, args: RankArgs) {
        let signer = self.signer();
        let address = if let Some(address) = args.address {
            Pubkey::from_str(&address).unwrap()
        } else {
            proof_pubkey(signer.pubkey())
        };

        // Verify proof exists
        let _proof = get_proof(&self.rpc_client, address).await;

        // Build and submit rank transaction
        let ix = ore_boost_api::sdk::rank(signer.pubkey(), address);
        self.send_and_confirm(&[ix], ComputeBudget::Fixed(50_000), false)
            .await
            .ok();
    }
}