use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use crate::config::{AppConfig, ConfigError, ConfigPaths};
use crate::template::{TemplateError, TemplateSet};
use crate::theme::{Theme, ThemeError};

#[derive(Debug)]
pub enum SettingsError {
    Config(ConfigError),
    Theme(ThemeError),
    Template(TemplateError),
}

impl std::fmt::Display for SettingsError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Config(error) => write!(formatter, "{error}"),
            Self::Theme(error) => write!(formatter, "{error}"),
            Self::Template(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for SettingsError {}

impl From<ConfigError> for SettingsError {
    fn from(error: ConfigError) -> Self {
        Self::Config(error)
    }
}

impl From<ThemeError> for SettingsError {
    fn from(error: ThemeError) -> Self {
        Self::Theme(error)
    }
}

impl From<TemplateError> for SettingsError {
    fn from(error: TemplateError) -> Self {
        Self::Template(error)
    }
}

pub struct ResolvedSettings {
    pub config: AppConfig,
    pub theme: Theme,
    pub templates: TemplateSet,
}

pub struct SettingsManager {
    paths: ConfigPaths,
    current: Arc<ResolvedSettings>,
    watched: Vec<WatchedPath>,
}

impl SettingsManager {
    pub fn load() -> Result<Self, SettingsError> {
        let paths = ConfigPaths::discover()?;
        let (current, watched) = load_resolved_settings(&paths)?;
        Ok(Self {
            paths,
            current: Arc::new(current),
            watched,
        })
    }

    pub fn current(&self) -> Arc<ResolvedSettings> {
        Arc::clone(&self.current)
    }

    pub fn reload_if_changed(&mut self) -> Result<Option<Arc<ResolvedSettings>>, SettingsError> {
        if !self.watched.iter().any(WatchedPath::has_changed) {
            return Ok(None);
        }

        let (current, watched) = load_resolved_settings(&self.paths)?;
        let current = Arc::new(current);
        self.current = Arc::clone(&current);
        self.watched = watched;
        Ok(Some(current))
    }
}

fn load_resolved_settings(
    paths: &ConfigPaths,
) -> Result<(ResolvedSettings, Vec<WatchedPath>), SettingsError> {
    let config = AppConfig::load(paths)?;
    let themes_dir = crate::config::resolve_relative_to_root(&paths.root, &config.theme.directory);
    let (theme, theme_paths) = Theme::load(paths, &config)?;
    let (templates, templates_dir) = TemplateSet::load(paths, &config)?;

    let settings = ResolvedSettings {
        config,
        theme,
        templates,
    };

    let mut watched = vec![WatchedPath::new(paths.config_file.clone())];
    watched.push(WatchedPath::new(themes_dir));
    for theme_path in theme_paths {
        watched.push(WatchedPath::new(theme_path));
    }
    watched.push(WatchedPath::new(templates_dir.clone()));
    watched.extend(discover_template_files(&templates_dir));

    Ok((settings, watched))
}

fn discover_template_files(directory: &PathBuf) -> Vec<WatchedPath> {
    let Ok(entries) = fs::read_dir(directory) else {
        return Vec::new();
    };

    entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .map(WatchedPath::new)
        .collect()
}

struct WatchedPath {
    path: PathBuf,
    modified: Option<SystemTime>,
}

impl WatchedPath {
    fn new(path: PathBuf) -> Self {
        Self {
            modified: modified_time(&path),
            path,
        }
    }

    fn has_changed(&self) -> bool {
        modified_time(&self.path) != self.modified
    }
}

fn modified_time(path: &PathBuf) -> Option<SystemTime> {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
}
