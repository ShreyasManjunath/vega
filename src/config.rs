use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

pub const CONFIG_SCHEMA_VERSION: u32 = 1;
pub const APP_DIR_NAME: &str = "vega";

#[derive(Debug)]
pub enum ConfigError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    ParseToml {
        path: PathBuf,
        source: toml::de::Error,
    },
    UnsupportedSchemaVersion {
        path: PathBuf,
        found: u32,
        expected: u32,
    },
    InvalidXdgHome,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(formatter, "failed to read {}: {source}", path.display())
            }
            Self::ParseToml { path, source } => {
                write!(formatter, "failed to parse {}: {source}", path.display())
            }
            Self::UnsupportedSchemaVersion {
                path,
                found,
                expected,
            } => write!(
                formatter,
                "{} uses unsupported schema_version {}; expected {}",
                path.display(),
                found,
                expected
            ),
            Self::InvalidXdgHome => write!(formatter, "unable to resolve XDG config home"),
        }
    }
}

impl std::error::Error for ConfigError {}

#[derive(Clone, Debug)]
pub struct ConfigPaths {
    pub root: PathBuf,
    pub config_file: PathBuf,
}

impl ConfigPaths {
    pub fn discover() -> Result<Self, ConfigError> {
        let root = config_root()?;
        Ok(Self {
            config_file: root.join("config.toml"),
            root,
        })
    }
}

fn config_root() -> Result<PathBuf, ConfigError> {
    if let Some(value) = env::var_os("XDG_CONFIG_HOME") {
        if value.is_empty() {
            return Err(ConfigError::InvalidXdgHome);
        }
        return Ok(PathBuf::from(value).join(APP_DIR_NAME));
    }

    let Some(home) = env::var_os("HOME") else {
        return Err(ConfigError::InvalidXdgHome);
    };
    Ok(PathBuf::from(home).join(".config").join(APP_DIR_NAME))
}

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub schema_version: u32,
    pub behavior: BehaviorConfig,
    pub runtime: RuntimeConfig,
    pub keybindings: KeybindingsConfig,
    pub theme: ThemeConfig,
    pub templates: TemplatesConfig,
    pub plugins: BTreeMap<String, toml::Value>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            schema_version: CONFIG_SCHEMA_VERSION,
            behavior: BehaviorConfig::default(),
            runtime: RuntimeConfig::default(),
            keybindings: KeybindingsConfig::default(),
            theme: ThemeConfig::default(),
            templates: TemplatesConfig::default(),
            plugins: BTreeMap::new(),
        }
    }
}

impl AppConfig {
    pub fn load(paths: &ConfigPaths) -> Result<Self, ConfigError> {
        let mut config = Self::default();
        if !paths.config_file.exists() {
            return Ok(config);
        }

        let text = fs::read_to_string(&paths.config_file).map_err(|source| ConfigError::Io {
            path: paths.config_file.clone(),
            source,
        })?;
        let overrides: PartialAppConfig =
            toml::from_str(&text).map_err(|source| ConfigError::ParseToml {
                path: paths.config_file.clone(),
                source,
            })?;

        if let Some(schema_version) = overrides.schema_version
            && schema_version != CONFIG_SCHEMA_VERSION
        {
            return Err(ConfigError::UnsupportedSchemaVersion {
                path: paths.config_file.clone(),
                found: schema_version,
                expected: CONFIG_SCHEMA_VERSION,
            });
        }

        config.apply(overrides);
        Ok(config)
    }

    fn apply(&mut self, partial: PartialAppConfig) {
        if let Some(behavior) = partial.behavior {
            self.behavior.apply(behavior);
        }
        if let Some(runtime) = partial.runtime {
            self.runtime.apply(runtime);
        }
        if let Some(keybindings) = partial.keybindings {
            self.keybindings.apply(keybindings);
        }
        if let Some(theme) = partial.theme {
            self.theme.apply(theme);
        }
        if let Some(templates) = partial.templates {
            self.templates.apply(templates);
        }
        if let Some(plugins) = partial.plugins {
            self.plugins.extend(plugins);
        }
    }
}

#[derive(Clone, Debug)]
pub struct BehaviorConfig {
    pub default_mode: String,
    pub hot_reload: bool,
    pub poll_interval_ms: u64,
}

impl Default for BehaviorConfig {
    fn default() -> Self {
        Self {
            default_mode: "apps".to_string(),
            hot_reload: true,
            poll_interval_ms: 400,
        }
    }
}

impl BehaviorConfig {
    fn apply(&mut self, partial: PartialBehaviorConfig) {
        if let Some(default_mode) = partial.default_mode {
            self.default_mode = default_mode;
        }
        if let Some(hot_reload) = partial.hot_reload {
            self.hot_reload = hot_reload;
        }
        if let Some(poll_interval_ms) = partial.poll_interval_ms {
            self.poll_interval_ms = poll_interval_ms.max(50);
        }
    }
}

#[derive(Clone, Debug)]
pub struct RuntimeConfig {
    pub limit: usize,
    pub debug: bool,
    pub fzf_binary: String,
    pub timeout_ms: u64,
    pub fzf_flags: Vec<String>,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            limit: 20,
            debug: false,
            fzf_binary: "fzf".to_string(),
            timeout_ms: 1500,
            fzf_flags: Vec::new(),
        }
    }
}

impl RuntimeConfig {
    fn apply(&mut self, partial: PartialRuntimeConfig) {
        if let Some(limit) = partial.limit {
            self.limit = limit.max(1);
        }
        if let Some(debug) = partial.debug {
            self.debug = debug;
        }
        if let Some(fzf_binary) = partial.fzf_binary {
            self.fzf_binary = fzf_binary;
        }
        if let Some(timeout_ms) = partial.timeout_ms {
            self.timeout_ms = timeout_ms.max(1);
        }
        if let Some(fzf_flags) = partial.fzf_flags {
            self.fzf_flags = fzf_flags;
        }
    }
}

#[derive(Clone, Debug)]
pub struct KeybindingsConfig {
    pub submit: String,
    pub cancel: String,
    pub select_next: String,
    pub select_prev: String,
}

impl Default for KeybindingsConfig {
    fn default() -> Self {
        Self {
            submit: "Enter".to_string(),
            cancel: "Escape".to_string(),
            select_next: "ArrowDown".to_string(),
            select_prev: "ArrowUp".to_string(),
        }
    }
}

impl KeybindingsConfig {
    fn apply(&mut self, partial: PartialKeybindingsConfig) {
        if let Some(submit) = partial.submit {
            self.submit = submit;
        }
        if let Some(cancel) = partial.cancel {
            self.cancel = cancel;
        }
        if let Some(select_next) = partial.select_next {
            self.select_next = select_next;
        }
        if let Some(select_prev) = partial.select_prev {
            self.select_prev = select_prev;
        }
    }
}

#[derive(Clone, Debug)]
pub struct ThemeConfig {
    pub name: String,
    pub directory: String,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            name: "catppuccin-mocha".to_string(),
            directory: "themes".to_string(),
        }
    }
}

impl ThemeConfig {
    fn apply(&mut self, partial: PartialThemeConfig) {
        if let Some(name) = partial.name {
            self.name = name;
        }
        if let Some(directory) = partial.directory {
            self.directory = directory;
        }
    }
}

#[derive(Clone, Debug)]
pub struct TemplatesConfig {
    pub enabled: bool,
    pub directory: String,
}

impl Default for TemplatesConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            directory: "templates".to_string(),
        }
    }
}

impl TemplatesConfig {
    fn apply(&mut self, partial: PartialTemplatesConfig) {
        if let Some(enabled) = partial.enabled {
            self.enabled = enabled;
        }
        if let Some(directory) = partial.directory {
            self.directory = directory;
        }
    }
}

#[derive(Debug, Deserialize)]
struct PartialAppConfig {
    schema_version: Option<u32>,
    behavior: Option<PartialBehaviorConfig>,
    runtime: Option<PartialRuntimeConfig>,
    keybindings: Option<PartialKeybindingsConfig>,
    theme: Option<PartialThemeConfig>,
    templates: Option<PartialTemplatesConfig>,
    plugins: Option<BTreeMap<String, toml::Value>>,
}

#[derive(Debug, Deserialize)]
struct PartialBehaviorConfig {
    default_mode: Option<String>,
    hot_reload: Option<bool>,
    poll_interval_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct PartialRuntimeConfig {
    limit: Option<usize>,
    debug: Option<bool>,
    fzf_binary: Option<String>,
    timeout_ms: Option<u64>,
    fzf_flags: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct PartialKeybindingsConfig {
    submit: Option<String>,
    cancel: Option<String>,
    select_next: Option<String>,
    select_prev: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PartialThemeConfig {
    name: Option<String>,
    directory: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PartialTemplatesConfig {
    enabled: Option<bool>,
    directory: Option<String>,
}

pub fn resolve_relative_to_root(root: &Path, value: &str) -> PathBuf {
    let path = PathBuf::from(value);
    if path.is_absolute() {
        return path;
    }
    root.join(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_config_overrides() {
        let mut config = AppConfig::default();
        config.apply(PartialAppConfig {
            schema_version: Some(CONFIG_SCHEMA_VERSION),
            behavior: Some(PartialBehaviorConfig {
                default_mode: Some("cmd".to_string()),
                hot_reload: Some(false),
                poll_interval_ms: Some(1250),
            }),
            runtime: Some(PartialRuntimeConfig {
                limit: Some(42),
                debug: Some(true),
                fzf_binary: Some("fzf-tmux".to_string()),
                timeout_ms: Some(3200),
                fzf_flags: Some(vec!["--algo=v2".to_string()]),
            }),
            keybindings: Some(PartialKeybindingsConfig {
                submit: Some("Space".to_string()),
                cancel: None,
                select_next: None,
                select_prev: Some("K".to_string()),
            }),
            theme: Some(PartialThemeConfig {
                name: Some("gruvbox-dark".to_string()),
                directory: None,
            }),
            templates: Some(PartialTemplatesConfig {
                enabled: Some(false),
                directory: Some("skins".to_string()),
            }),
            plugins: Some(BTreeMap::from([(
                "demo".to_string(),
                toml::Value::String("enabled".to_string()),
            )])),
        });

        assert_eq!(config.behavior.default_mode, "cmd");
        assert!(!config.behavior.hot_reload);
        assert_eq!(config.runtime.limit, 42);
        assert_eq!(config.runtime.fzf_binary, "fzf-tmux");
        assert_eq!(config.keybindings.submit, "Space");
        assert_eq!(config.keybindings.select_prev, "K");
        assert_eq!(config.theme.name, "gruvbox-dark");
        assert!(!config.templates.enabled);
        assert_eq!(config.templates.directory, "skins");
        assert!(config.plugins.contains_key("demo"));
    }
}
