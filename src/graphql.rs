pub use graphql_client::GraphQLQuery;
pub use mint_to_collection::*;
pub use retry_mint_to_collection::*;
use serde::{Deserialize, Serialize};

pub use mint_status::CreationStatus;

#[allow(clippy::upper_case_acronyms)]
pub type UUID = uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct GraphQLError {
    pub message: String,
    pub _path: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct GraphQLResponse<T> {
    pub data: Option<T>,
    pub errors: Option<Vec<GraphQLError>>,
}

#[derive(Debug, Deserialize, GraphQLQuery)]
#[graphql(
    schema_path = "holaplex.graphql",
    query_path = "queries/mint_compressed.graphql",
    response_derives = "Debug, Deserialize, Serialize"
)]
pub struct MintToCollection;

#[derive(Debug, Deserialize, GraphQLQuery)]
#[graphql(
    schema_path = "holaplex.graphql",
    query_path = "queries/retry_mint.graphql",
    response_derives = "Debug, Deserialize, Serialize"
)]
pub struct RetryMintToCollection;

#[derive(Debug, Deserialize, GraphQLQuery)]
#[graphql(
    schema_path = "holaplex.graphql",
    query_path = "queries/mint_status.graphql",
    response_derives = "Debug, Deserialize, Serialize, Clone, PartialEq"
)]
pub struct MintStatus;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RetryMintToCollectionData {
    #[serde(rename = "collectionMint")]
    pub collection_mint: CollectionMint,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MintToCollectionData {
    #[serde(rename = "collectionMint")]
    pub collection_mint: CollectionMint,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MintResponse {
    #[serde(rename = "mintToCollection")]
    pub mint_to_collection: MintToCollectionData,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RetryMintResponse {
    #[serde(rename = "retryMintToCollection")]
    pub retry_mint_to_collection: RetryMintToCollectionData,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CollectionMint {
    pub id: String,
    #[serde(rename = "creationStatus")]
    pub creation_status: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MintStatusResponse {
    pub mint: MintData,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MintData {
    pub id: String,
    #[serde(rename = "creationStatus")]
    pub creation_status: CreationStatus,
}
