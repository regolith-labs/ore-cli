use std::str::FromStr;

use ore_api::state::proof_pda;
use ore_boost_api::{state::boost_pda, consts::BOOST_DENOMINATOR};
use solana_client::client_error::Result as ClientResult;
use steel::*;

use crate::{args::BoostArgs, Miner, utils::{get_boosts, get_boost, get_proof}};

impl Miner {
    pub async fn boost(&self, args: BoostArgs) -> ClientResult<()> {
        if let Some(mint) = args.mint {
            let mint = Pubkey::from_str(&mint).unwrap();
            self.lookup_boost(mint).await;
        } else {
            self.list_boosts().await;
        }
        Ok(())
    }

    async fn lookup_boost(&self, mint: Pubkey) {
        let boost_address = boost_pda(mint).0;
        let boost = get_boost(&self.rpc_client, boost_address).await;
        let proof_address = proof_pda(boost_address).0;
        let proof = get_proof(&self.rpc_client, proof_address).await;
        println!("Address: {:?}", boost_address);
        println!("Expires at: {:?}", boost.expires_at);
        println!("Mint: {:?}", mint);
        println!("Multiplier: {:?}", boost.multiplier);
        println!("Total stake: {:?}", boost.total_stake);
        println!("Pending yield: {:?}", proof.balance);
    }

    async fn list_boosts(&self) {
        let boosts = get_boosts(&self.rpc_client).await.unwrap();
        for (_address, boost) in boosts {
            println!("{:?} ({:?})", boost.mint, boost.multiplier as f64 / BOOST_DENOMINATOR as f64);
        }
    }
}
