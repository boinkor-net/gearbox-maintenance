mod metrics;

use metrics::*;

use anyhow::{anyhow, Context, Result};
use gearbox_maintenance::{
    config::{Config, Instance},
    Torrent,
};
use std::{collections::HashMap, convert::TryFrom, io, net::SocketAddr, path::PathBuf, sync::Arc};
use structopt::StructOpt;
use tokio::{task, time};
use tracing::{debug, info, warn};
use transmission_rpc::{
    types::{BasicAuth, Id},
    TransClient,
};

#[derive(StructOpt)]
struct Opt {
    /// The config file to load
    config: PathBuf,

    #[structopt(short = "f")]
    /// Actually perform policy actions
    take_action: bool,

    #[structopt(long)]
    /// Serve prometheus metrics on this network address
    prometheus_listen_addr: Option<SocketAddr>,
}

fn init_logging() {
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_writer(io::stderr)
        .with_env_filter("gearbox_maintenance=info")
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
}

async fn tick_on_instance(instance: &Instance, take_action: bool) -> Result<()> {
    let _timer = TICK_DURATION
        .get_metric_with_label_values(&[&instance.transmission.url])?
        .start_timer();
    let status = FailureCounter::new(
        TICK_FAILURES.get_metric_with_label_values(&[&instance.transmission.url])?,
    );
    let url = instance.transmission.url.to_string();
    let basic_auth = BasicAuth {
        user: instance
            .transmission
            .user
            .clone()
            .unwrap_or_else(|| "".to_string()),
        password: instance
            .transmission
            .password
            .clone()
            .unwrap_or_else(|| "".to_string()),
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
    let mut counts: HashMap<String, usize> = Default::default();
    let mut sizes: HashMap<String, usize> = Default::default();
    for torrent in all_torrents {
        for (index, policy) in instance.policies.iter().enumerate() {
            let is_match = policy.match_when.matches_torrent(&torrent);
            if is_match.is_real_mismatch() {
                continue;
            }
            counts
                .entry(policy.name_or_index(index).into_owned())
                .and_modify(|n| *n += 1)
                .or_insert(1);
            sizes
                .entry(policy.name_or_index(index).into_owned())
                .and_modify(|n| *n += torrent.total_size)
                .or_insert(torrent.total_size);
            TORRENT_SIZE_HIST
                .get_metric_with_label_values(&[
                    &instance.transmission.url,
                    policy.name_or_index(index).as_ref(),
                ])?
                .observe(torrent.total_size as f64);
            if is_match.is_match() {
                TORRENT_DELETIONS
                    .get_metric_with_label_values(&[
                        &instance.transmission.url,
                        policy.name_or_index(index).as_ref(),
                    ])?
                    .inc();
                if !take_action {
                    info!(
                        "Would delete {}: matches {} on {}",
                        torrent.name,
                        is_match,
                        policy.name_or_index(index),
                    );
                } else {
                    info!(
                        "Will delete {}: matches {} on {}",
                        torrent.name,
                        is_match,
                        policy.name_or_index(index)
                    );
                }
                if policy.delete_data {
                    delete_ids_with_data.push(Id::Hash(torrent.hash.to_string()));
                } else {
                    delete_ids_without_data.push(Id::Hash(torrent.hash.to_string()));
                }
            }
        }
    }
    for (policy_name, count) in counts.iter() {
        TORRENT_COUNT
            .get_metric_with_label_values(&[&instance.transmission.url, policy_name])?
            .set(*count as f64);
    }
    for (policy_name, size) in sizes.iter() {
        TORRENT_SIZES
            .get_metric_with_label_values(&[&instance.transmission.url, policy_name])?
            .set(*size as f64);
    }

    if take_action {
        if !delete_ids_with_data.is_empty() {
            info!(
                "Deleting data for {} torrents...",
                delete_ids_with_data.len()
            );
            client
                .torrent_remove(delete_ids_with_data, true)
                .await
                .map_err(|e| anyhow!(e.to_string()))
                .context("Deleting torrents with local data")?;
        }
        if !delete_ids_without_data.is_empty() {
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
    status.succeed();
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let opt = Opt::from_args();

    init_logging();
    let mut handles: Vec<_> = Config::configure(&opt.config)?
        .into_iter()
        .map(|instance| {
            info!(
                "Running on instance {:?}, polling every {:?}",
                instance.transmission.url, instance.transmission.poll_interval
            );
            task::spawn(async move {
                let mut ticker =
                    time::interval(instance.transmission.poll_interval.to_std().unwrap());
                loop {
                    debug!("Polling {}", instance.transmission.url);
                    if let Err(e) = tick_on_instance(&instance, opt.take_action).await {
                        warn!("Error polling {}: {}", instance.transmission.url, e);
                    } else {
                        debug!("Polling {} succeeded", instance.transmission.url);
                    }
                    ticker.tick().await;
                }
            })
        })
        .collect();

    if let Some(addr) = opt.prometheus_listen_addr {
        let shutdown = futures::future::pending();
        handles.push(task::spawn(async move {
            prometheus_hyper::Server::run(
                Arc::new(prometheus::default_registry().clone()),
                addr,
                shutdown,
            )
            .await
        }));
        info!("Serving prometheus metrics on http://{}/metrics", addr);
    }
    for handle in handles {
        handle.await??;
    }
    Ok(())
}
