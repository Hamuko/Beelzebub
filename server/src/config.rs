    use log::debug;
use serde::Deserialize;
use shared;
use std::fs::File;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum Error {
    /// Could not deserialise the Yaml.
    DeserialisationError(serde_yaml::Error),

    /// Could not determine from where to load the settings.
    DirectoryError,

    /// IO error with the configuration.
    IOError(std::io::Error),
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub db_url: String,

    pub secret: Option<String>,
}

impl Config {
    pub fn get_path() -> Result<PathBuf, Error> {
        let Some(project_directory) = directories::ProjectDirs::from(
            shared::CONFIG_QUALIFIER,
            shared::CONFIG_ORGANIZATION,
            shared::CONFIG_APPLICATION,
        ) else {
            return Err(Error::DirectoryError);
        };
        let mut config_path = PathBuf::new();
        config_path.push(project_directory.config_dir());
        config_path.push("server.yaml");
        return Ok(config_path);
    }

    pub fn load(config_path: &Path) -> Result<Self, Error> {
        debug!("Loading config from {}", config_path.display());
        let fp = File::open(&config_path).map_err(Error::IOError)?;
        let config: Config = serde_yaml::from_reader(fp).map_err(Error::DeserialisationError)?;
        return Ok(config);
    }
}
