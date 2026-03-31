use serde::Deserialize;
use std::path::PathBuf;

/// Configuration loaded from `~/.config/portmap/config.toml`.
/// All fields are optional; missing values fall back to hardcoded defaults.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Port the dashboard listens on.
    pub listen: Option<u16>,
    /// Port scan range start (inclusive).
    pub scan_start: Option<u16>,
    /// Port scan range end (inclusive).
    pub scan_end: Option<u16>,
}

const DEFAULT_LISTEN: u16 = 1337;
const DEFAULT_SCAN_START: u16 = 1000;
const DEFAULT_SCAN_END: u16 = 9999;

impl Config {
    pub fn listen(&self) -> u16 {
        self.listen.unwrap_or(DEFAULT_LISTEN)
    }

    pub fn scan_start(&self) -> u16 {
        self.scan_start.unwrap_or(DEFAULT_SCAN_START)
    }

    pub fn scan_end(&self) -> u16 {
        self.scan_end.unwrap_or(DEFAULT_SCAN_END)
    }
}

/// Return the portmap config directory: `~/.config/portmap`.
pub fn config_dir() -> PathBuf {
    dirs_fallback().join("portmap")
}

/// Return the config file path: `~/.config/portmap/config.toml`.
pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

/// New default database path: `~/.config/portmap/portmap.db`.
pub fn default_db_path() -> PathBuf {
    config_dir().join("portmap.db")
}

/// Old database path: `~/.portmap.db`.
fn old_db_path() -> Option<PathBuf> {
    home_dir().map(|h| h.join(".portmap.db"))
}

/// Load config from `~/.config/portmap/config.toml`.
/// Returns the default config if the file does not exist or cannot be parsed.
pub fn load() -> Config {
    let path = config_path();
    match std::fs::read_to_string(&path) {
        Ok(contents) => match toml::from_str::<Config>(&contents) {
            Ok(cfg) => cfg,
            Err(e) => {
                eprintln!("Warning: failed to parse {}: {e}", path.display());
                Config::default()
            }
        },
        Err(_) => Config::default(),
    }
}

/// Resolve the database path, handling migration from the old location.
///
/// Precedence: CLI `--database` flag (if not the sentinel default) > new default path.
///
/// If the new default path does not exist but the old `~/.portmap.db` does,
/// the old file is copied to the new location and a message is printed.
pub fn resolve_db_path(cli_database: &str) -> String {
    // If the user explicitly passed --database with a non-default value, use it.
    if cli_database != DEFAULT_DB_SENTINEL {
        return shellexpand(cli_database);
    }

    let new_path = default_db_path();

    // Ensure the config directory exists.
    if let Some(parent) = new_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    if new_path.exists() {
        return new_path.display().to_string();
    }

    // Check for the old database location and migrate.
    if let Some(old) = old_db_path()
        && old.exists()
    {
        match std::fs::copy(&old, &new_path) {
            Ok(_) => {
                eprintln!(
                    "Migrated database: {} -> {}",
                    old.display(),
                    new_path.display()
                );
                // Remove old file after successful copy.
                let _ = std::fs::remove_file(&old);
            }
            Err(e) => {
                eprintln!(
                    "Warning: could not migrate database to {}: {e}. Using old path.",
                    new_path.display()
                );
                return old.display().to_string();
            }
        }
    }

    new_path.display().to_string()
}

/// Sentinel value used as clap default so we can detect "user did not pass --database".
pub const DEFAULT_DB_SENTINEL: &str = "__portmap_default__";

fn shellexpand(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/")
        && let Some(home) = home_dir()
    {
        return format!("{}/{rest}", home.display());
    }
    path.to_string()
}

fn home_dir() -> Option<PathBuf> {
    std::env::var("HOME").ok().map(PathBuf::from)
}

/// `~/.config` fallback (avoids pulling in the `dirs` crate).
fn dirs_fallback() -> PathBuf {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(xdg);
    }
    home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_defaults() {
        let cfg = Config::default();
        assert_eq!(cfg.listen(), 1337);
        assert_eq!(cfg.scan_start(), 1000);
        assert_eq!(cfg.scan_end(), 9999);
    }

    #[test]
    fn test_parse_toml() {
        let toml_str = r#"
listen = 8080
scan_start = 2000
scan_end = 5000
"#;
        let cfg: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.listen(), 8080);
        assert_eq!(cfg.scan_start(), 2000);
        assert_eq!(cfg.scan_end(), 5000);
    }

    #[test]
    fn test_partial_toml() {
        let toml_str = "listen = 9999\n";
        let cfg: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.listen(), 9999);
        assert_eq!(cfg.scan_start(), 1000); // default
        assert_eq!(cfg.scan_end(), 9999); // default
    }
}
