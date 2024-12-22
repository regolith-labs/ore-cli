use ore_boost_api::state::reservation_pda;
use solana_sdk::signature::Signer;

use crate::{send_and_confirm::ComputeBudget, utils::proof_pubkey, Miner};

impl Miner {
    pub async fn open(&self) {
        // Register miner
        let mut ixs = Vec::new();
        let signer = self.signer();
        let fee_payer = self.fee_payer();
        let proof_address = proof_pubkey(signer.pubkey());
        if self.rpc_client.get_account(&proof_address).await.is_err() {
            let ix = ore_api::sdk::open(signer.pubkey(), signer.pubkey(), fee_payer.pubkey());
            ixs.push(ix);
        }

        // Register reservation
        let reservation_address = reservation_pda(proof_address).0;
        if self.rpc_client.get_account(&reservation_address).await.is_err() {
            let ix = ore_boost_api::sdk::register(signer.pubkey(), fee_payer.pubkey(), reservation_address);
            ixs.push(ix);
        }

        // Submit transaction
        if ixs.len() > 0 {
            self.send_and_confirm(&ixs, ComputeBudget::Fixed(400_000), false)
                .await
                .ok();
        }
    }
}
