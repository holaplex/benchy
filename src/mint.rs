use std::{str::FromStr, time::Instant};

use anyhow::{anyhow, Result};
use log::{debug, error, info};
use uuid::Uuid;

use crate::{config::Config, graphql::*, HubClient};

#[derive(Clone)]
pub struct State {
    pub start_time: Instant,
    pub last_pending_time: Instant,
    pub retry_count: u64,
}

pub async fn execute(hub: &HubClient) -> Result<CollectionMint> {
    let config = Config::read();
    let mc = config.mint.clone();
    let mutation = MintToCollection::build_query(mint_to_collection::Variables {
        input: MintToCollectionInput {
            collection: Uuid::from_str(&mc.collection_id).unwrap(),
            recipient: mc.recipient.clone(),
            seller_fee_basis_points: Some(0),
            compressed: Some(mc.compressed),
            creators: vec![CreatorInput {
                address: mc.creator.address.clone(),
                share: 100,
                verified: Some(mc.creator.verified),
            }],
            metadata_json: MetadataJsonInput {
                name: format!("{:.18}", Uuid::new_v4().to_string()),
                symbol: "HOLAPLEX".to_string(),
                description: mc.description.clone(),
                collection: None,
                animation_url: None,
                external_url: None,
                properties: None,
                image: mc.image.clone(),
                attributes: vec![MetadataJsonAttributeInput {
                    trait_type: "Benchmark".to_string(),
                    value: "true".to_string(),
                }],
            },
        },
    });

    let res_plain = hub
        .client
        .post(hub.url.clone())
        .json(&mutation)
        .send()
        .await?
        .text()
        .await?;

    process_response(&res_plain, |data: MintResponse| {
        let cm = data.mint_to_collection.collection_mint.clone();
        info!(
            "Mint req sent successfully: MintID: {} -- Status: {}",
            cm.id, cm.creation_status
        );
        Ok(CollectionMint {
            id: cm.id,
            creation_status: cm.creation_status,
        })
    })
}

pub async fn retry(hub: &HubClient, id: Uuid) -> Result<CollectionMint> {
    let mutation = RetryMintToCollection::build_query(retry_mint_to_collection::Variables {
        input: RetryMintEditionInput { id },
    });
    let res_plain = hub
        .client
        .post(hub.url.clone())
        .json(&mutation)
        .send()
        .await?
        .text()
        .await?;

    process_response(&res_plain, |data: RetryMintResponse| {
        let cm = data.retry_mint_to_collection.collection_mint.clone();
        info!(
            "Retry Mint req sent successfully: MintID: {} -- Status: {}",
            cm.id, cm.creation_status
        );
        Ok(CollectionMint {
            id: cm.id,
            creation_status: cm.creation_status,
        })
    })
}

pub async fn check_status(hub: &HubClient, id: Uuid) -> Result<MintData> {
    let query = MintStatus::build_query(mint_status::Variables { id });
    let res_plain = hub
        .client
        .post(hub.url.clone())
        .json(&query)
        .send()
        .await?
        .text()
        .await?;

    process_response(&res_plain, |data: MintStatusResponse| {
        let cm = data.mint;
        debug!(
            "Checking status of mint {} -- Status: {:?}",
            cm.id, cm.creation_status
        );
        if cm.creation_status == CreationStatus::CREATED {
            info!("Mint {} created successfully", cm.id);
        }
        Ok(cm)
    })
}

fn process_response<T, R>(res_plain: &str, on_success: impl FnOnce(T) -> Result<R>) -> Result<R>
where
    T: serde::de::DeserializeOwned,
{
    match serde_json::from_str::<GraphQLResponse<T>>(res_plain) {
        Ok(GraphQLResponse {
            errors: Some(errors),
            ..
        }) => {
            error!("{}", res_plain);
            let messages: Vec<_> = errors.iter().map(|e| &e.message).collect();
            Err(anyhow!("GraphQL Errors: {:?}", messages))
        },
        Ok(GraphQLResponse {
            data: Some(data), ..
        }) => on_success(data),
        Ok(_) | Err(_) => {
            let e = format!(
                "Unable to parse response. Operation failed with error: {}",
                res_plain
            );
            error!("{e}");
            Err(anyhow!("{e}"))
        },
    }
}
