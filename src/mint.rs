use std::str::FromStr;

use anyhow::{anyhow, Result};
use log::{debug, error, info};
use uuid::Uuid;

use crate::{config::Config, graphql::*, HubClient};

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

    let response = hub
        .client
        .post(hub.url.clone())
        .json(&mutation)
        .send()
        .await?;

    let res_plain = response.text().await?;
    debug!("{res_plain}");
    let res: GraphQLResponse<MintResponse> = serde_json::from_str(&res_plain)?;

    if let Some(errors) = res.errors {
        let messages: Vec<_> = errors.iter().map(|e| &e.message).collect();
        error!("{}", res_plain);
        return Err(anyhow!("GraphQL Errors: {:?}", messages));
    }

    let data = if let Some(data) = res.data {
        let cm = data.mint_to_collection.collection_mint.clone();
        info!(
            "Mint req sent successfully: MintID: {} -- Status: {}",
            cm.id, cm.creation_status
        );
        data
    } else {
        error!("Data is missing from the response");
        return Err(anyhow!("Data is missing from the response"));
    };

    Ok(CollectionMint {
        id: data.mint_to_collection.collection_mint.id,
        creation_status: data.mint_to_collection.collection_mint.creation_status,
    })
}

pub async fn retry(hub: &HubClient, id: Uuid) -> Result<CollectionMint> {
    let mutation = RetryMintToCollection::build_query(retry_mint_to_collection::Variables {
        input: RetryMintEditionInput { id },
    });

    let response = hub
        .client
        .post(hub.url.clone())
        .json(&mutation)
        .send()
        .await?;

    let res_plain = response.text().await?;
    let res: GraphQLResponse<RetryMintResponse> = serde_json::from_str(&res_plain)?;

    if let Some(errors) = res.errors {
        let messages: Vec<_> = errors.iter().map(|e| &e.message).collect();
        error!("{}", res_plain);
        return Err(anyhow!("GraphQL Errors: {:?}", messages));
    }

    let data = if let Some(data) = res.data {
        let cm = data.retry_mint_to_collection.collection_mint.clone();
        info!(
            "Retry Mint req sent successfully: MintID: {} -- Status: {}",
            cm.id, cm.creation_status
        );
        data
    } else {
        error!("Data is missing from the response");
        return Err(anyhow!("Data is missing from the response"));
    };

    Ok(CollectionMint {
        id: data.retry_mint_to_collection.collection_mint.id,
        creation_status: data
            .retry_mint_to_collection
            .collection_mint
            .creation_status,
    })
}

pub async fn check_status(hub: &HubClient, id: Uuid) -> Result<MintData> {
    let query = MintStatus::build_query(mint_status::Variables { id });

    let response = hub.client.post(hub.url.clone()).json(&query).send().await?;
    let res_plain = response.text().await?;
    let res: GraphQLResponse<MintStatusResponse> = serde_json::from_str(&res_plain)?;

    match res {
        GraphQLResponse {
            errors: Some(errors),
            ..
        } => {
            error!("{}", res_plain);
            let messages: Vec<_> = errors.iter().map(|e| &e.message).collect();
            Err(anyhow!("GraphQL Errors: {:?}", messages))
        },
        GraphQLResponse {
            data: Some(data), ..
        } => {
            let cm = data.mint.clone();
            debug!(
                "Checking status of mint {} -- Status: {:?}",
                cm.id, cm.creation_status
            );
            if cm.creation_status == CreationStatus::CREATED {
                info!("Mint {} created successfully", cm.id)
            };
            Ok(cm)
        },
        _ => {
            error!("Data is missing from the response");
            Err(anyhow!("Data is missing from the response"))
        },
    }
}
