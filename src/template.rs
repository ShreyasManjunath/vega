use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use minijinja::{Environment, context};

use crate::candidate::Candidate;
use crate::config::{AppConfig, ConfigPaths, resolve_relative_to_root};

#[derive(Debug)]
pub enum TemplateError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    InvalidTemplateName {
        path: PathBuf,
    },
    Compile {
        template: String,
        source: minijinja::Error,
    },
    Render {
        template: String,
        source: minijinja::Error,
    },
}

impl std::fmt::Display for TemplateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(
                    formatter,
                    "failed to read template {}: {source}",
                    path.display()
                )
            }
            Self::InvalidTemplateName { path } => {
                write!(formatter, "invalid template filename {}", path.display())
            }
            Self::Compile { template, source } => {
                write!(
                    formatter,
                    "failed to compile template `{template}`: {source}"
                )
            }
            Self::Render { template, source } => {
                write!(
                    formatter,
                    "failed to render template `{template}`: {source}"
                )
            }
        }
    }
}

impl std::error::Error for TemplateError {}

pub struct TemplateSet {
    enabled: bool,
    env: Environment<'static>,
}

impl TemplateSet {
    pub fn load(paths: &ConfigPaths, config: &AppConfig) -> Result<(Self, PathBuf), TemplateError> {
        let templates_dir = resolve_relative_to_root(&paths.root, &config.templates.directory);
        let mut env = Environment::new();

        for (name, source) in builtin_templates() {
            env.add_template_owned(name.to_string(), source.to_string())
                .map_err(|source| TemplateError::Compile {
                    template: name.to_string(),
                    source,
                })?;
        }

        if config.templates.enabled && templates_dir.exists() {
            for entry in fs::read_dir(&templates_dir).map_err(|source| TemplateError::Io {
                path: templates_dir.clone(),
                source,
            })? {
                let entry = entry.map_err(|source| TemplateError::Io {
                    path: templates_dir.clone(),
                    source,
                })?;
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }

                let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
                    return Err(TemplateError::InvalidTemplateName { path });
                };

                let source = fs::read_to_string(&path).map_err(|source| TemplateError::Io {
                    path: path.clone(),
                    source,
                })?;
                env.add_template_owned(stem.to_string(), source)
                    .map_err(|source| TemplateError::Compile {
                        template: stem.to_string(),
                        source,
                    })?;
            }
        }

        Ok((
            Self {
                enabled: config.templates.enabled,
                env,
            },
            templates_dir,
        ))
    }

    pub fn render_mode_badge(&self, mode_name: &str) -> String {
        self.render("mode_badge", context! { mode_name => mode_name }, mode_name)
    }

    pub fn render_empty_state(&self, query: &str) -> String {
        self.render("empty_state", context! { query => query }, "No matches")
    }

    pub fn render_row_primary(&self, candidate: &Candidate) -> String {
        self.render_candidate("row_primary", candidate, &candidate.primary)
    }

    pub fn render_row_secondary(&self, candidate: &Candidate) -> String {
        candidate
            .secondary
            .as_deref()
            .map_or_else(String::new, |secondary| {
                self.render_candidate("row_secondary", candidate, secondary)
            })
    }

    fn render_candidate(&self, template: &str, candidate: &Candidate, fallback: &str) -> String {
        self.render(
            template,
            context! {
                id => candidate.id.to_string(),
                primary => candidate.primary.as_str(),
                secondary => candidate.secondary.as_deref().unwrap_or(""),
            },
            fallback,
        )
    }

    fn render(&self, template: &str, ctx: minijinja::Value, fallback: &str) -> String {
        if !self.enabled {
            return fallback.to_string();
        }

        self.env
            .get_template(template)
            .ok()
            .and_then(|template_ref| template_ref.render(ctx).ok())
            .filter(|rendered| !rendered.is_empty())
            .unwrap_or_else(|| fallback.to_string())
    }
}

fn builtin_templates() -> BTreeMap<&'static str, &'static str> {
    BTreeMap::from([
        ("mode_badge", "{{ mode_name|upper }}"),
        (
            "empty_state",
            "{% if query %}No matches{% else %}Start typing{% endif %}",
        ),
        ("row_primary", "{{ primary }}"),
        ("row_secondary", "{{ secondary }}"),
    ])
}
