mod schema;

pub use schema::*;

use crate::error::{CraiError, CraiResult};
use std::path::Path;

pub fn load_config(path: &Path) -> CraiResult<Config> {
    if !path.exists() {
        return Err(CraiError::ConfigNotFound(path.to_path_buf()));
    }

    let content = std::fs::read_to_string(path)?;
    let config: Config = toml::from_str(&content).map_err(|e| CraiError::ConfigParse {
        path: path.to_path_buf(),
        source: e,
    })?;

    Ok(config)
}

pub fn load_config_or_default(path: &Path) -> Config {
    load_config(path).unwrap_or_default()
}

pub fn create_default_config(path: &Path) -> CraiResult<()> {
    let config = Config::default();
    let content = toml::to_string_pretty(&config)
        .map_err(|e| CraiError::Serialization(e.to_string()))?;
    std::fs::write(path, content)?;
    Ok(())
}
