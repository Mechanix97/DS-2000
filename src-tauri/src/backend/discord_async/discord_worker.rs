use crate::error::DiscordError;

use spawned_concurrency::{
    messages::Unused,
    tasks::{CastResponse, GenServer, GenServerHandle, send_after},
};
use std::time::Duration;
const DISCORD_FETCH_INTERVAL: u64 = 100;

type DiscordWorkerHandler = GenServerHandle<DiscordWorker>;

pub enum DiscordWorkerState {
    NotConnected,
    HandshakeDone,
    Authenticated,
    Connected,
}

#[derive(Clone)]
pub enum InMessage {
    Fetch,
}

#[derive(Clone, PartialEq)]
pub enum OutMessage {
    Done,
}

#[derive(Clone)]
pub struct DiscordWorker {
    fetch_interval_ms: u64,
}

impl DiscordWorker {
    pub fn new() -> Self {
        Self {
            fetch_interval_ms: DISCORD_FETCH_INTERVAL,
        }
    }

    pub async fn spawn() -> DiscordWorkerHandler {
        let state = Self::new();
        state.start()
    }
}

impl GenServer for DiscordWorker {
    type CallMsg = Unused;
    type CastMsg = InMessage;
    type OutMsg = OutMessage;
    type Error = DiscordError;

    async fn handle_cast(
        mut self,
        message: Self::CastMsg,
        handle: &GenServerHandle<Self>,
    ) -> CastResponse<Self> {
        match message {
            InMessage::Fetch => {
                eprintln!("HOLA");
                send_after(
                    Duration::from_millis(self.fetch_interval_ms),
                    handle.clone(),
                    Self::CastMsg::Fetch,
                );
                CastResponse::NoReply(self)
            }
        }
        // if let SequencerStatus::Syncing = self.sequencer_state.status().await {
        //     let _ = self.fetch().await.inspect_err(|err| {
        //         error!("Block Fetcher Error: {err}");
        //     });
        // }
    }
}

// for running these tests, discord should be running on the background
#[cfg(test)]
mod tests {
    use super::DiscordWorker;
    use super::DiscordWorkerHandler;
    use super::InMessage;

    use tokio::time::{Duration, sleep};

    #[tokio::test]
    async fn test_discord_worker_connection() {
        let mut dw: DiscordWorkerHandler = DiscordWorker::spawn().await;
        dw.cast(InMessage::Fetch).await.unwrap();
        sleep(Duration::from_secs(5)).await;

        assert!(false);
    }
}
