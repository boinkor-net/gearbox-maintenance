use anyhow::{anyhow, Context, Result};
use gearbox_maintenance::{config::Config, Torrent};
use log::{debug, info};
use std::{convert::TryFrom, path::PathBuf};
use structopt::StructOpt;
use transmission_rpc::{
    types::{BasicAuth, Id},
    TransClient,
};

#[derive(StructOpt)]
struct Opt {
    /// The config file to load
    config: PathBuf,

    /// Actually perform policy actions
    #[structopt(short = "f")]
    take_action: bool,
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
        debug!("Running on instance {:?}", instance);

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

        let mut delete_ids_with_data: Vec<Id> = Default::default();
        let mut delete_ids_without_data: Vec<Id> = Default::default();
        for torrent in all_torrents {
            for policy in instance.policies.iter() {
                let is_match = policy.match_when.matches_torrent(&torrent);
                if is_match.is_match() {
                    if !opt.take_action {
                        info!("Would delete {}: matches {}", torrent.name, is_match);
                    } else {
                        debug!("Will delete {}: matches {}", torrent.name, is_match);
                    }
                    if policy.delete_data {
                        delete_ids_with_data.push(Id::Hash(torrent.hash.to_string()));
                    } else {
                        delete_ids_without_data.push(Id::Hash(torrent.hash.to_string()));
                    }
                }
            }
        }
        if opt.take_action {
            info!(
                "Deleting data for {} torrents...",
                delete_ids_with_data.len()
            );
            client
                .torrent_remove(delete_ids_with_data, true)
                .await
                .map_err(|e| anyhow!(e.to_string()))
                .context("Deleting torrents with local data")?;

            info!(
                "Deleting torrents without data for {} torrents...",
                delete_ids_without_data.len()
            );
            client
                .torrent_remove(delete_ids_without_data, true)
                .await
                .map_err(|e| anyhow!(e.to_string()))
                .context("Deleting torrent metadata alone")?;
        }
    }

    Ok(())
}
