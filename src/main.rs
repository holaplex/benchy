use std::{collections::HashMap, fs::File, str::FromStr, sync::Arc, time::Instant};

use anyhow::Result;
use futures::stream::{FuturesUnordered, StreamExt};
use graphql::CreationStatus;
use indicatif_log_bridge::LogWrapper;
use log::{error, info};
use structopt::StructOpt;
use tokio::{sync::Semaphore, time::Duration};
use uuid::Uuid;

use crate::{
    cli::Opt,
    config::{Config, Settings},
    csv::{Record, Writer},
    hub::HubClient,
    pbs::{MultiProgress, ProgressBar},
};

mod cli;
mod config;
mod csv;
mod graphql;
mod hub;
mod mint;
mod pbs;

#[derive(Clone)]
struct MintState {
    start_time: Instant,
    last_pending_time: Instant,
    retry_count: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Opt::from_args();

    Config::load(&cli.global.config)?;
    let cfg = Config::read();
    let settings = Settings::merge(cfg.settings.clone(), &cli.clone());
    let level = settings.log_level.clone().unwrap();
    let logger =
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(&level)).build();
    let multi = MultiProgress::new();
    LogWrapper::new(multi.clone(), logger).try_init().unwrap();
    let hub = HubClient::new(&cfg.hub)?;
    run(hub, &settings, multi).await
}

async fn run(hub: HubClient, s: &Settings, m: MultiProgress) -> Result<()> {
    let mut wtr = Writer::from_path("output.csv").unwrap();
    let semaphore = Arc::new(Semaphore::new(s.parallelism.unwrap()));

    let total_mints = s.iterations.unwrap() * s.parallelism.unwrap();

    let pbs = pbs::init(&m, total_mints, s.retry.unwrap_or_default()).await;

    let mints = mint(&hub, s, &semaphore, &pbs["mints"]).await?;
    pbs["mints"].finish_with_message("All mint requests sent!");

    if s.iterations.unwrap() < 2 {
        info!("Waiting 10 seconds before starting mint status verification");
        tokio::time::sleep(Duration::from_millis(10000)).await;
    };

    let records = verify(&hub, &mints, s.retry.unwrap_or_default(), &pbs).await;

    pbs::finalize(&pbs["successful"], &records).await;

    save(&mut wtr, &records)?;
    Ok(())
}

async fn mint(
    hub: &HubClient,
    s: &Settings,
    semaphore: &Arc<Semaphore>,
    pb: &ProgressBar,
) -> Result<HashMap<Uuid, Instant>> {
    let mut mints = HashMap::new();

    for i in 0..s.iterations.unwrap() {
        let mut all_futures = FuturesUnordered::new();

        for _ in 0..s.parallelism.unwrap() {
            let semaphore_clone = semaphore.clone();
            let hub = hub.clone();
            let start_time = Instant::now();
            all_futures.push(async move {
                let _ = semaphore_clone.acquire_owned().await;
                let result = mint::execute(&hub).await;
                pb.inc(1);
                (result, start_time)
            });
        }

        while let Some((mint_result, start_time)) = all_futures.next().await {
            if let Ok(mint) = mint_result {
                let mint_id = Uuid::from_str(&mint.id).unwrap();
                mints.insert(mint_id, start_time);
            }
        }

        if i < s.iterations.unwrap() - 1 {
            tokio::time::sleep(Duration::from_secs(s.delay.unwrap())).await;
        }
    }

    Ok(mints)
}

async fn handle_mint(
    hub: &HubClient,
    mint_id: Uuid,
    state: &mut MintState,
    retry: bool,
    pbs: &HashMap<&'static str, ProgressBar>,
) -> Option<Record> {
    match mint::check_status(hub, mint_id).await {
        Ok(updated_mint_data) => match updated_mint_data.creation_status {
            CreationStatus::CREATED => {
                pbs["successful"].inc(1);
                Some(Record {
                    mint_id,
                    completion_sec: state.start_time.elapsed().as_secs(),
                    retry_count: state.retry_count,
                    success: true,
                })
            },
            CreationStatus::FAILED => {
                pbs["failed"].inc(1);
                if retry {
                    let _ = mint::retry(hub, mint_id).await;
                    info!("Retrying mint {} due to FAILED status", mint_id);
                    state.retry_count += 1;
                    state.last_pending_time = Instant::now();
                    pbs["retries"].inc(1);
                }
                None
            },
            _ => None,
        },
        Err(e) => {
            error!("Failed to verify mint {}: {:?}", mint_id, e);
            None
        },
    }
}

async fn verify(
    hub: &HubClient,
    mints: &HashMap<Uuid, Instant>,
    retry: bool,
    pbs: &HashMap<&'static str, ProgressBar>,
) -> Vec<Record> {
    let mut records = Vec::new();
    let pending_timeout = tokio::time::Duration::from_secs(400); // 40 * 10 seconds
    let retry_delay = tokio::time::Duration::from_secs(10);

    let mut pending_states: HashMap<Uuid, MintState> = mints
        .iter()
        .map(|(id, &time)| {
            (*id, MintState {
                start_time: time,
                last_pending_time: time,
                retry_count: 0,
            })
        })
        .collect();

    while !pending_states.is_empty() {
        let mut to_remove = Vec::new();

        for (&mint_id, state) in &mut pending_states {
            if state.last_pending_time.elapsed() > pending_timeout {
                error!(
                    "Mint {} is still pending after {} seconds",
                    mint_id,
                    pending_timeout.as_secs()
                );
                records.push(Record {
                    mint_id,
                    completion_sec: state.start_time.elapsed().as_secs(),
                    retry_count: state.retry_count,
                    success: false,
                });
                to_remove.push(mint_id);
            } else if let Some(record) = handle_mint(hub, mint_id, state, retry, pbs).await {
                records.push(record);
                to_remove.push(mint_id);
            }
        }

        for mint_id in to_remove {
            pending_states.remove(&mint_id);
        }

        tokio::time::sleep(retry_delay).await;
    }

    records
}

fn save(wtr: &mut Writer<File>, records: &Vec<Record>) -> Result<()> {
    for record in records {
        wtr.serialize(record)?;
    }
    wtr.flush()?;
    Ok(())
}
