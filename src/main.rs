mod metrics;

use metrics::*;

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use gearbox_maintenance::{
    config::{configure, Instance},
    Torrent,
};
use prometheus_client::registry::Registry;
use std::{collections::HashMap, convert::TryFrom, io, net::SocketAddr, path::PathBuf};
use tokio::task::JoinSet;
use tokio::time;
use tracing::{debug, info, metadata::LevelFilter, warn};
use tracing_subscriber::EnvFilter;
use transmission_rpc::{
    types::{BasicAuth, Id},
    TransClient,
};
use url::Url;

#[derive(Parser, Debug)]
#[clap(author = "Andreas Fuchs <asf@boinkor.net>")]
struct Opt {
    /// The config file to load
    config: PathBuf,

    #[clap(short = 'f')]
    /// Actually perform policy actions
    take_action: bool,

    #[clap(long)]
    /// Serve prometheus metrics on this network address
    prometheus_listen_addr: Option<SocketAddr>,
}

fn init_logging() {
    let filter = EnvFilter::from_default_env()
        .add_directive(LevelFilter::INFO.into())
        .add_directive(
            "transmission_rpc=warn"
                .parse()
                .expect("'filter out transmission-rpc"),
        );
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_writer(io::stderr)
        .with_env_filter(filter)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
}

#[tracing::instrument(skip(instance), fields(instance=instance.transmission.url))]
async fn tick_on_instance(instance: &Instance, take_action: bool, metrics: &Metrics) -> Result<()> {
    let _tick_timer = metrics.tick_duration(&instance.transmission.url);
    let status = metrics.tick_failure_tracker(&instance.transmission.url);
    let url = Url::parse(&instance.transmission.url)?;
    let basic_auth = BasicAuth {
        user: instance.transmission.user.clone().unwrap_or_default(),
        password: instance.transmission.password.clone().unwrap_or_default(),
    };
    let mut client = TransClient::with_auth(url, basic_auth);
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
            let metrics_policy = Policy::new_for(
                &instance.transmission.url,
                policy.name_or_index(index).as_ref(),
            );
            let is_match = policy.applicable(&torrent).map(|a| a.matches());
            if is_match.is_none() {
                // This torrent is not interesting to us
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
            metrics.track_size(&metrics_policy, torrent.total_size);
            if let Some(true) = is_match.map(|cm| cm.is_match()) {
                metrics.track_torrent_deletion(&metrics_policy);
                info!(
                    torrent = ?torrent.name,
                    matched_policy = ?policy.name_or_index(index),
                    ?take_action,
                    delete_data = ?policy.delete_data,
                    "Matched torrent",
                );

                if policy.delete_data {
                    delete_ids_with_data.push(Id::Hash(torrent.hash.to_string()));
                } else {
                    delete_ids_without_data.push(Id::Hash(torrent.hash.to_string()));
                }
            }
        }
    }
    for (policy_name, count) in counts.iter() {
        metrics.update_count(
            &Policy::new_for(&instance.transmission.url, policy_name),
            *count,
        );
    }
    for (policy_name, size) in sizes.iter() {
        metrics.update_size(
            &Policy::new_for(&instance.transmission.url, policy_name),
            *size,
        );
    }

    if take_action {
        if !delete_ids_with_data.is_empty() {
            info!(
                torrents_to_delete = delete_ids_with_data.len(),
                "Deleting data..."
            );
            client
                .torrent_remove(delete_ids_with_data, true)
                .await
                .map_err(|e| anyhow!(e.to_string()))
                .context("Deleting torrents with local data")?;
        }
        if !delete_ids_without_data.is_empty() {
            info!(
                torrents_to_delete = delete_ids_without_data.len(),
                "Deleting torrents without data.."
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
    let opt = Opt::parse();
    let mut metrics_registry = Registry::default();
    let metrics = Metrics::for_registry(&mut metrics_registry);

    init_logging();
    // let instances = StarlarkConfig::configure(&opt.config)?;
    let instances = configure(&opt.config).map_err(|e| anyhow!("{e}"))?;
    let mut handles = JoinSet::new();
    for instance in instances {
        info!(
            instance=instance.transmission.url, poll_interval=?instance.transmission.poll_interval,
            "Running"
        );
        let metrics = metrics.clone();
        handles.spawn(async move {
            let mut ticker =
                time::interval(instance.transmission.poll_interval.to_std().unwrap());
            loop {
                ticker.tick().await;
                debug!(instance=instance.transmission.url, "Polling");
                if let Err(e) = tick_on_instance(&instance, opt.take_action, &metrics).await {
                    warn!(instance=instance.transmission.url, error=%e, error_debug=?e, "Error polling");
                } else {
                    debug!(instance=instance.transmission.url, "Polling succeeded");
                }
            }
        });
    }

    if let Some(addr) = opt.prometheus_listen_addr {
        handles.spawn(async move {
            let router = metrics::metrics_router(metrics_registry);
            let listener = tokio::net::TcpListener::bind(addr)
                .await
                .map_err(|e| format!("Could not listen on metrics address {:?}: {}", addr, e))
                .expect("Listen on metrics address");
            axum::serve(listener, router).await
        });
        info!(
            metrics_endpoint = format!("http://{}/metrics", addr),
            "Serving prometheus metrics"
        );
    }
    // Any of these tasks returning is bad news:
    if let Some(task) = handles.join_next().await {
        task?.context("task exited prematurely")?;
        anyhow::bail!("Task exited unexpectedly, but with a success?");
    }
    Ok(())
}
