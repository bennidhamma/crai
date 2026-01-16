mod schema;

pub use schema::*;

use crate::error::{CraiError, CraiResult};
use std::path::{Path, PathBuf};

/// Get the OS-appropriate config directory for crai
/// - Linux: ~/.config/crai
/// - macOS: ~/Library/Application Support/crai
/// - Windows: %APPDATA%\crai
pub fn config_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("crai"))
}

/// Get the default config file path
pub fn default_config_path() -> Option<PathBuf> {
    config_dir().map(|p| p.join("crai.toml"))
}

/// Check if the config file exists in the user's config directory
pub fn config_exists() -> bool {
    default_config_path()
        .map(|p| p.exists())
        .unwrap_or(false)
}

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

/// Load config from the default user config directory, or return None if not found
pub fn load_user_config() -> Option<Config> {
    default_config_path()
        .and_then(|p| load_config(&p).ok())
}

pub fn create_default_config(path: &Path) -> CraiResult<()> {
    let config = Config::default();
    save_config(path, &config)
}

/// Create config with a specific AI provider
pub fn create_config_with_provider(path: &Path, provider: AiProviderType) -> CraiResult<()> {
    let mut config = Config::default();
    config.ai.provider = provider;
    save_config(path, &config)
}

/// Save config to a file, creating parent directories if needed
pub fn save_config(path: &Path, config: &Config) -> CraiResult<()> {
    // Create parent directories if they don't exist
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let content = toml::to_string_pretty(config)
        .map_err(|e| CraiError::Serialization(e.to_string()))?;
    std::fs::write(path, content)?;
    Ok(())
}
