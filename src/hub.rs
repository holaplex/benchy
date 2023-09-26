use anyhow::Result;
use reqwest::{header, Client};

use crate::config::Hub;

#[derive(Clone)]
pub struct HubClient {
    pub client: Client,
    pub url: String,
}

impl HubClient {
    pub fn new(config: &Hub) -> Result<Self> {
        let mut headers = header::HeaderMap::new();
        let header_value = header::HeaderValue::from_str(&config.token.clone())
            .map_err(|_| "Invalid header value")
            .unwrap();
        headers.insert("Authorization", header_value);
        let hub_client = reqwest::Client::builder()
            .default_headers(headers)
            .build()?;

        Ok(Self {
            client: hub_client,
            url: config.url.to_string(),
        })
    }
}
