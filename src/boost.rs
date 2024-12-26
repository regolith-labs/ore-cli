use std::str::FromStr;

use ore_boost_api::{state::{boost_pda, Boost}, consts::BOOST_DENOMINATOR};
use solana_client::client_error::Result as ClientResult;
use steel::*;

use crate::{args::BoostArgs, Miner, utils::get_boosts};

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
        let Ok(data) = self.rpc_client.get_account_data(&boost_address).await else {
            println!("No boost found for mint {:?}", mint);
            return;
        };
        let boost = Boost::try_from_bytes(&data).unwrap();
        println!("Address: {:?}", boost_address);
        println!("Expires at: {:?}", boost.expires_at);
        println!("Mint: {:?}", mint);
        println!("Multiplier: {:?}", boost.multiplier);
        println!("Total stake: {:?}", boost.total_stake);
    }

    async fn list_boosts(&self) {
        let boosts = get_boosts(&self.rpc_client).await.unwrap();
        for (_address, boost) in boosts {
            println!("{:?} ({:?})", boost.mint, boost.multiplier as f64 / BOOST_DENOMINATOR as f64);
        }
    }
}
