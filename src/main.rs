use anyhow::{anyhow, Result};
use dotenv::dotenv;
use gearbox_maintenance::Torrent;
use log::info;
use std::{convert::TryFrom, env};
use transmission_rpc::{types::BasicAuth, TransClient};

fn init_logging() {
    let env = env_logger::Env::default().filter_or("RUST_LOG", "gearbox_maintenance=info");
    env_logger::init_from_env(env);
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    init_logging();
    let url = env::var("TURL")?;
    let basic_auth = BasicAuth {
        user: env::var("TUSER").unwrap_or("".to_string()),
        password: env::var("TPWD").unwrap_or("".to_string()),
    };
    let client = TransClient::with_auth(&url, basic_auth);
    let all_torrents: Vec<Torrent> = client
        .torrent_get(Torrent::request_fields(), None)
        .await
        .map_err(|e| anyhow!("Could not retrieve list of torrents: {}", e))?
        .arguments
        .torrents
        .into_iter()
        .map(Torrent::try_from)
        .collect::<Result<_, anyhow::Error>>()?;
    let failed_torrents: Vec<&Torrent> = all_torrents.iter().filter(|t| !t.is_ok()).collect();
    info!("torrents in error states: {:?}", failed_torrents);

    let high_ratios: Vec<&Torrent> = all_torrents
        .iter()
        .filter(|t| t.upload_ratio > 1.01)
        .collect();
    info!("torrents with high ratios: {:?}", high_ratios);
    Ok(())
}
