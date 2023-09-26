use std::{
    collections::HashMap,
    fs::File,
    str::FromStr,
    sync::{Arc, Mutex},
    time::Instant,
};

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
    mint::State,
    pbs::{MultiProgress, ProgressBar},
};

mod cli;
mod config;
mod csv;
mod graphql;
mod hub;
mod mint;
mod pbs;

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
    let wtr = Writer::from_path(cli.global.output).unwrap();
    run(hub, &settings, multi, wtr).await
}

async fn run(hub: HubClient, s: &Settings, m: MultiProgress, mut wtr: Writer<File>) -> Result<()> {
    let semaphore = Arc::new(Semaphore::new(s.parallelism.unwrap()));
    let retry_delay = s.retry_delay.unwrap_or(10);
    let total_mints = s.iterations.unwrap() * s.parallelism.unwrap();

    let pbs = pbs::init(&m, total_mints, s.retry.unwrap_or_default()).await;

    let mints = mint(&hub, s, &semaphore, &pbs["mints"]).await?;
    pbs["mints"].finish_with_message("All mint requests sent!");

    if s.iterations.unwrap() < 2 {
        info!("Waiting {retry_delay} seconds before starting mint status verification");
        tokio::time::sleep(Duration::from_secs(retry_delay)).await;
    };

    let records = verify(&hub, &mints, s, &pbs).await;

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

    for _ in 0..s.iterations.unwrap_or(0) {
        let results: Vec<_> = (0..s.parallelism.unwrap_or(0))
            .map(|_| {
                let semaphore_clone = semaphore.clone();
                let hub = hub.clone();
                async move {
                    let _guard = semaphore_clone.acquire_owned().await;
                    let start_time = Instant::now();
                    let result = mint::execute(&hub).await;
                    pb.inc(1);
                    (result, start_time)
                }
            })
            .collect::<FuturesUnordered<_>>()
            .collect()
            .await;

        for (mint_result, start_time) in results {
            if let Ok(mint) = mint_result {
                let mint_id = Uuid::from_str(&mint.id).unwrap();
                mints.insert(mint_id, start_time);
            }
        }

        if let Some(delay) = s.delay {
            tokio::time::sleep(Duration::from_secs(delay)).await;
        }
    }

    Ok(mints)
}

async fn handle_status(
    hub: &HubClient,
    mint_id: Uuid,
    state: &mut State,
    retry: bool,
    pbs: &HashMap<&'static str, ProgressBar>,
) -> Option<Record> {
    match mint::check_status(hub, mint_id).await {
        Ok(updated_mint_data) => match updated_mint_data.creation_status {
            CreationStatus::CREATED => Some(Record {
                mint_id,
                completion_sec: state.start_time.elapsed().as_secs(),
                retry_count: state.retry_count,
                success: true,
                reason: String::new(),
            }),
            CreationStatus::FAILED => {
                if retry {
                    let _ = mint::retry(hub, mint_id).await;
                    pbs["retries"].inc(1);
                    info!("Retrying FAILED mint {mint_id}");
                    state.retry_count += 1;
                }
                state.last_pending_time = Instant::now();
                Some(Record {
                    mint_id,
                    completion_sec: state.start_time.elapsed().as_secs(),
                    retry_count: state.retry_count,
                    success: false,
                    reason: "backend was unable to mint".to_string(),
                })
            },
            _ => None,
        },
        Err(e) => {
            let msg = format!("Failed to verify mint {}: {:?}", mint_id, e);
            error!("{msg}");
            Some(Record {
                mint_id,
                completion_sec: state.start_time.elapsed().as_secs(),
                retry_count: state.retry_count,
                success: false,
                reason: msg,
            })
        },
    }
}

async fn verify(
    hub: &HubClient,
    mints: &HashMap<Uuid, Instant>,
    s: &Settings,
    pbs: &HashMap<&'static str, ProgressBar>,
) -> Vec<Record> {
    let pending_timeout = tokio::time::Duration::from_secs(s.timeout.unwrap_or(400));
    let retry_delay = tokio::time::Duration::from_secs(s.retry_delay.unwrap_or(10));
    let mut records = Vec::new();

    let mut pending_states: HashMap<Uuid, Arc<Mutex<State>>> = mints
        .iter()
        .map(|(id, &time)| {
            (
                *id,
                Arc::new(Mutex::new(State {
                    start_time: time,
                    last_pending_time: time,
                    retry_count: 0,
                })),
            )
        })
        .collect();

    while records.len() < mints.len() {
        let futures: FuturesUnordered<_> = pending_states
            .iter()
            .map(|(&mint_id, state)| {
                let state = state.clone();
                let hub = hub.clone();
                let pbs = pbs.clone();
                let retry = s.retry.unwrap_or(false);

                async move {
                    let mut locked_state = state.lock().unwrap();
                    if locked_state.last_pending_time.elapsed() > pending_timeout {
                        let msg = format!(
                            "Mint {} is still pending after {} seconds",
                            mint_id,
                            pending_timeout.as_secs()
                        );
                        error!("{msg}");
                        Some(Record {
                            mint_id,
                            completion_sec: locked_state.start_time.elapsed().as_secs(),
                            retry_count: locked_state.retry_count,
                            success: false,
                            reason: msg,
                        })
                    } else {
                        handle_status(&hub, mint_id, &mut locked_state, retry, &pbs).await
                    }
                }
            })
            .collect();

        let results: Vec<_> = futures.collect().await;
        for record in results.into_iter().flatten() {
            if record.success {
                pbs["successful"].inc(1);
                pending_states.remove(&record.mint_id);
            } else if !record.success && !s.retry.unwrap_or(false) {
                pbs["failed"].inc(1);
                pending_states.remove(&record.mint_id);
            }
            records.push(record);
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

    let cli = Opt::from_args();
    info!("Report saved to {}", cli.global.output.display());
    Ok(())
}
