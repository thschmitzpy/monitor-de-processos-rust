use crate::app::{self, AppState, SortDir, SortKey};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub sort_key: SortKey,
    #[serde(default)]
    pub sort_dir: SortDir,
    #[serde(default = "default_refresh_ms")]
    pub refresh_ms: u64,
}

fn default_refresh_ms() -> u64 {
    app::DEFAULT_REFRESH_MS
}

impl Default for Config {
    fn default() -> Self {
        Self {
            sort_key: SortKey::default(),
            sort_dir: SortDir::default(),
            refresh_ms: default_refresh_ms(),
        }
    }
}

impl Config {
    pub fn from_state(state: &AppState) -> Self {
        Self {
            sort_key: state.sort_key,
            sort_dir: state.sort_dir,
            refresh_ms: state.refresh_ms,
        }
    }
}

pub fn config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("monitorando").join("config.toml"))
}

pub fn load() -> Config {
    let Some(path) = config_path() else {
        return Config::default();
    };
    match std::fs::read_to_string(&path) {
        Ok(contents) => toml::from_str(&contents).unwrap_or_default(),
        Err(_) => Config::default(),
    }
}

pub fn save(config: &Config) -> std::io::Result<()> {
    let Some(path) = config_path() else {
        return Ok(());
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let contents = toml::to_string_pretty(config)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    std::fs::write(path, contents)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_matches_default_app_state() {
        let cfg = Config::default();
        let state = AppState::new();
        assert_eq!(cfg.sort_key, state.sort_key);
        assert_eq!(cfg.sort_dir, state.sort_dir);
        assert_eq!(cfg.refresh_ms, state.refresh_ms);
    }

    #[test]
    fn roundtrip_serialize_deserialize() {
        let cfg = Config {
            sort_key: SortKey::Cpu,
            sort_dir: SortDir::Desc,
            refresh_ms: 500,
        };
        let s = toml::to_string(&cfg).expect("serialize");
        let back: Config = toml::from_str(&s).expect("deserialize");
        assert_eq!(cfg, back);
    }

    #[test]
    fn empty_toml_yields_defaults() {
        let cfg: Config = toml::from_str("").expect("parse empty");
        assert_eq!(cfg, Config::default());
    }

    #[test]
    fn partial_config_fills_missing_with_defaults() {
        let cfg: Config = toml::from_str(r#"sort_key = "Cpu""#).expect("parse partial");
        assert_eq!(cfg.sort_key, SortKey::Cpu);
        assert_eq!(cfg.sort_dir, SortDir::Asc);
        assert_eq!(cfg.refresh_ms, app::DEFAULT_REFRESH_MS);
    }

    #[test]
    fn from_state_captures_persistable_fields() {
        let mut s = AppState::new();
        s.sort_key = SortKey::Memory;
        s.sort_dir = SortDir::Desc;
        s.refresh_ms = 2000;
        let cfg = Config::from_state(&s);
        assert_eq!(cfg.sort_key, SortKey::Memory);
        assert_eq!(cfg.sort_dir, SortDir::Desc);
        assert_eq!(cfg.refresh_ms, 2000);
    }

    #[test]
    fn ignores_unknown_fields_gracefully() {
        let cfg: Config = toml::from_str(
            r#"
            sort_key = "Io"
            refresh_ms = 250
            "#,
        )
        .expect("parse");
        assert_eq!(cfg.sort_key, SortKey::Io);
        assert_eq!(cfg.refresh_ms, 250);
        assert_eq!(cfg.sort_dir, SortDir::Asc);
    }
}
