use std::str::FromStr;

use ore_boost_api::state::{boost_pda, Boost};
use solana_client::client_error::Result as ClientResult;
use steel::*;

use crate::{args::BoostArgs, Miner};

impl Miner {
    pub async fn boost(&self, args: BoostArgs) -> ClientResult<()> {
        let mint = Pubkey::from_str(&args.mint).unwrap();
        let boost_address = boost_pda(mint).0;
        let Ok(data) = self.rpc_client.get_account_data(&boost_address).await else {
            println!("No boost found for mint {:?}", mint);
            return Ok(());
        };
        let boost = Boost::try_from_bytes(&data).unwrap();
        println!("Address: {:?}", boost_address);
        println!("Expires at: {:?}", boost.expires_at);
        println!("Mint: {:?}", mint);
        println!("Multiplier: {:?}", boost.multiplier);
        println!("Total stake: {:?}", boost.total_stake);
        Ok(())
    }
}
