use std::fmt::Display;

use config::{Config, ConfigError, File};
use serde::Deserialize;

use crate::PeriphVariantConfig;

#[derive(Deserialize, Debug)]
pub struct Chip {
    name: String,
    peripherals: Vec<Feature>,
}

impl Chip {
    pub fn get_config() -> Result<Self, ConfigError> {
        let variant = Self::get_variant();
        let config_file = crate::config_dir()
            .join("chips")
            .join(format!("{variant}.json"));

        Config::builder()
            .add_source(File::with_name(config_file.to_str().unwrap()))
            .build()?
            .try_deserialize()
    }

    pub fn get_variant() -> String {
        let mut variants = Vec::new();
        #[cfg(feature = "samd21j")]
        variants.push("samd21j".to_owned());

        #[cfg(feature = "samd51j")]
        variants.push("samd51j".to_owned());

        assert!(
            variants.len() == 1,
            "You must enable only one chip variant Cargo feature. Possible options are: {}",
            ChipsList::new()
        );

        variants.remove(0)
    }

    pub fn peripheral_variant_matches(&self, periph_config: &PeriphVariantConfig) -> bool {
        let chip_config = self
            .peripherals
            .iter()
            .find(|p| *p.name == periph_config.peripheral);

        let Some(chip_config) = chip_config else {
            return false;
        };

        chip_config.variant == periph_config.variant
    }
}

#[derive(Deserialize, Debug)]
pub struct Feature {
    name: String,
    variant: String,
}

struct ChipsList {
    chips: Vec<String>,
}

impl ChipsList {
    fn new() -> Self {
        let config_files = crate::config_dir().join("chips");
        let chips = std::fs::read_dir(config_files)
            .unwrap()
            .map(|f| {
                f.unwrap()
                    .path()
                    .file_stem()
                    .unwrap()
                    .to_owned()
                    .into_string()
                    .unwrap()
            })
            .collect();

        Self { chips }
    }
}

impl Display for ChipsList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f)?;
        for chip in self.chips.iter() {
            writeln!(f, "\t- {chip}")?;
        }

        Ok(())
    }
}
