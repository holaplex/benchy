use std::{fs, io, path::PathBuf, sync::Arc};

use anyhow::Result;
use log::error;
use once_cell::sync::OnceCell;
use serde::Deserialize;
use url::Url;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub hub: Hub,
    pub settings: Settings,
    pub mint: MintConfig,
}
#[derive(Debug, Deserialize, Clone)]
pub struct Settings {
    pub parallelism: Option<usize>,
    pub iterations: Option<usize>,
    pub delay: Option<u64>,
}
#[derive(Debug, Deserialize)]
pub struct Hub {
    pub url: Url,
    pub token: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MintConfig {
    pub collection_id: String,
    pub recipient: String,
    pub creator: CreatorConfig,
    pub description: String,
    pub compressed: bool,
    pub image: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CreatorConfig {
    pub address: String,
    pub verified: bool,
}

static CONFIG: OnceCell<Arc<Config>> = OnceCell::new();

impl Config {
    /// # Errors
    ///
    /// Will return `Err` if unable to read config file
    pub fn load(path: &PathBuf) -> Result<(), io::Error> {
        let config = serde_json::from_str::<Config>(&fs::read_to_string(path)?)?;
        CONFIG
            .set(Arc::new(config))
            .map_err(|_| io::Error::new(io::ErrorKind::AlreadyExists, "Config already loaded"))?;
        Ok(())
    }

    pub fn read() -> &'static Config {
        CONFIG.get().map_or_else(
            || {
                error!("Unable to read config. Exiting");
                std::process::exit(1)
            },
            std::convert::AsRef::as_ref,
        )
    }
}

impl Settings {
    pub fn merge(self, cli: &crate::Opt) -> Self {
        let mut settings = self.clone();

        if let Some(cmd) = &cli.cmd {
            settings.parallelism = settings.parallelism.or(Some(cmd.parallelism));
            settings.iterations = settings.iterations.or(Some(cmd.iterations));
            settings.delay = settings.delay.or(Some(cmd.delay));
        }
        settings
    }
}
