use std::str::FromStr;

use solana_sdk::{pubkey::Pubkey, signature::Signer, transaction::Transaction};

use crate::{args::BoostArgs, Miner};

impl Miner {
    pub async fn boost(&self, args: BoostArgs) {
        let mint = Pubkey::from_str(&args.mint).unwrap();
        let blockhash = self.rpc_client.get_latest_blockhash().await.unwrap();
        let ix = ore_boost_api::instruction::new(self.signer().pubkey(), mint, args.multiplier);
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.signer().pubkey()),
            &[&self.signer()],
            blockhash,
        );
        let res = self.rpc_client.send_and_confirm_transaction(&tx).await;
        println!("{:?}", res);
    }
}
