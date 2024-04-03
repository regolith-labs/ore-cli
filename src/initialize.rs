use ore::TREASURY_ADDRESS;
use solana_sdk::signature::Signer;

use crate::{send_and_confirm::CU_LIMIT_UNINFORMED_GUESSWORK_TO_MAKE_COMPILER_HAPPY, Miner};

impl Miner {
    pub async fn initialize(&self) {
        // Return early if program is initialized
        let signer = self.signer();
        if (&self.rpc_client)
            .get_account(&TREASURY_ADDRESS)
            .await
            .is_ok()
        {
            return;
        }

        // Sign and send transaction.
        let ix = ore::instruction::initialize(signer.pubkey());
        self.send_and_confirm(&[ix], CU_LIMIT_UNINFORMED_GUESSWORK_TO_MAKE_COMPILER_HAPPY)
            .await
            .expect("Transaction failed");
    }
}
