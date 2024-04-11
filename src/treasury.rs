use std::{sync::Arc, time::Duration};

use ore::{state::Treasury, utils::AccountDeserialize, TREASURY_ADDRESS};
use solana_client::nonblocking::rpc_client::RpcClient;
use tokio::sync::RwLock;

pub struct TreasuryService;

const POLL_MILLIS: u64 = 20_000;

impl TreasuryService {
    pub fn start(rpc: Arc<RpcClient>) -> LatestTreasury {
        // Polling interval
        let mut interval = tokio::time::interval(Duration::from_millis(POLL_MILLIS));

        // Arc value
        let treasury_arc = Arc::new(RwLock::new(None));
        let treasury_arc_ = treasury_arc.clone();

        // Spawn polling task
        tokio::task::spawn(async move {
            let mut attempts = 0;
            loop {
                interval.tick().await;

                // Fetch data
                let Ok(data) = rpc.get_account_data(&TREASURY_ADDRESS).await else {
                    attempts += 1;
                    logfather::warn!("failed to fetch treasury ({})", attempts);
                    continue;
                };

                // Update arc
                if let Ok(treasury) = Treasury::try_from_bytes(&data) {
                    *treasury_arc_.write().await = Some(treasury.clone());
                }

                // Reset attempts
                attempts = 0;
            }
        });

        LatestTreasury(treasury_arc)
    }
}

#[derive(Clone)]
pub struct LatestTreasury(Arc<RwLock<Option<Treasury>>>);

impl LatestTreasury {
    pub async fn load(&self) -> Option<Treasury> {
        *self.0.read().await
    }
}
