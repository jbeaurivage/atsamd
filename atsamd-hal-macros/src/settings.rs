use std::collections::HashMap;

use config::{ConfigError, File};
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub(crate) struct Config {
    peripherals: HashMap<String, Peripheral>,
}

#[derive(Deserialize, Debug)]
pub(crate) struct Peripheral {
    variants: Vec<String>,
}

impl Config {
    pub fn new() -> Result<Self, ConfigError> {
        let settings = config::Config::builder()
            .add_source(File::with_name(
                crate::config_dir().join("config.toml").to_str().unwrap(),
            ))
            .build()?;

        settings.try_deserialize()
    }
}
