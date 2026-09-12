use serde::Deserialize;
use std::path::PathBuf;

#[derive(Deserialize, Debug, Clone)]
pub struct WatchEntry {
    pub path: String,
    #[serde(default = "default_recursive")]
    pub recursive: bool,
}

fn default_recursive() -> bool {
    true
}

fn default_baseline_path() -> String {
    "~/.local/state/sigilward/baseline.json".to_string()
}

#[derive(Deserialize, Debug, Clone)]
pub struct AppConfig {
    #[serde(default = "default_baseline_path")]
    pub baseline_path: String,
    pub watch: Vec<WatchEntry>,
}

pub fn default_config_path() -> Option<PathBuf> {
    let config_home = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|_| std::env::var("HOME").map(|h| PathBuf::from(h).join(".config")))
        .ok()?;
    let candidates = [config_home.join("sigilward").join("config.toml"), PathBuf::from("config.toml")];
    candidates.into_iter().find(|p| p.exists())
}

pub fn expand_home(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(path)
}

pub fn load(path: &std::path::Path) -> Result<AppConfig, String> {
    let raw = std::fs::read_to_string(path).map_err(|e| format!("failed to read {}: {}", path.display(), e))?;
    toml::from_str(&raw).map_err(|e| format!("failed to parse {}: {}", path.display(), e))
}
