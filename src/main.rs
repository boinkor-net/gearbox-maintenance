use anyhow::{anyhow, Result};
use chrono::{Duration, Utc};
use gearbox_maintenance::{config::Config, Torrent};
use log::info;
use std::{convert::TryFrom, path::PathBuf};
use structopt::StructOpt;
use transmission_rpc::{types::BasicAuth, TransClient};

#[derive(StructOpt)]
struct Opt {
    /// The config file to load
    config: PathBuf,
}

fn init_logging() {
    let env = env_logger::Env::default().filter_or("RUST_LOG", "gearbox_maintenance=info");
    env_logger::init_from_env(env);
}

#[tokio::main]
async fn main() -> Result<()> {
    let opt = Opt::from_args();

    init_logging();
    for instance in Config::configure(&opt.config)? {
        let url = instance.transmission.url;
        let basic_auth = BasicAuth {
            user: instance.transmission.user.unwrap_or("".to_string()),
            password: instance.transmission.password.unwrap_or("".to_string()),
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

        let olds: Vec<&Torrent> = all_torrents
            .iter()
            .filter(|t| {
                t.done_date
                    .map(|done| Utc::now() - done > Duration::hours(120))
                    == Some(true)
            })
            .collect();
        info!("torrents with long seed times: {:?}", olds);
    }

    Ok(())
}
