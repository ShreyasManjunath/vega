use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::{AppConfig, ConfigPaths, resolve_relative_to_root};

pub const THEME_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub fn to_egui(self) -> eframe::egui::Color32 {
        eframe::egui::Color32::from_rgba_unmultiplied(self.r, self.g, self.b, self.a)
    }
}

#[derive(Clone, Debug)]
pub struct Theme {
    pub name: String,
    pub schema_version: u32,
    pub window_background: Color,
    pub panel_padding: i8,
    pub item_spacing_x: f32,
    pub item_spacing_y: f32,
    pub header_gap: f32,
    pub row_gap: f32,
    pub header_height: f32,
    pub badge_width: f32,
    pub badge_background: Color,
    pub badge_foreground: Color,
    pub badge_radius: u8,
    pub badge_padding_x: i8,
    pub badge_padding_y: i8,
    pub badge_font_size: f32,
    pub input_background: Color,
    pub input_foreground: Color,
    pub input_placeholder_foreground: Color,
    pub input_padding_x: i8,
    pub input_padding_y: i8,
    pub input_font_size: f32,
    pub row_background: Color,
    pub row_hover_background: Color,
    pub row_selected_background: Color,
    pub row_foreground: Color,
    pub row_secondary_foreground: Color,
    pub row_primary_font_size: f32,
    pub row_secondary_font_size: f32,
    pub row_height: f32,
    pub row_padding_x: f32,
    pub row_padding_y: f32,
    pub empty_foreground: Color,
    pub empty_font_size: f32,
    pub error_foreground: Color,
    pub heading_font_size: f32,
    pub body_font_size: f32,
    pub button_font_size: f32,
    pub small_font_size: f32,
}

impl Theme {
    pub fn builtin(name: &str) -> Option<Self> {
        let source = builtin_theme_source(name)?;
        let parsed = parse_theme(name, source).ok()?;
        let mut theme = Theme::base(name);
        apply_sections(&mut theme, &parsed.sections, Path::new(name)).ok()?;
        Some(theme)
    }

    pub fn load(
        paths: &ConfigPaths,
        config: &AppConfig,
    ) -> Result<(Self, Vec<PathBuf>), ThemeError> {
        let name = config.theme.name.as_str();
        if let Some(theme) = Self::builtin(name) {
            return Ok((theme, Vec::new()));
        }

        let themes_dir = resolve_relative_to_root(&paths.root, &config.theme.directory);
        let theme_path = resolve_theme_reference(&themes_dir, name)
            .ok_or_else(|| ThemeError::UnknownTheme(name.to_string()))?;
        let mut loaded_paths = Vec::new();
        let theme = load_theme_file(paths, &themes_dir, &theme_path, name, &mut loaded_paths)?;
        loaded_paths.sort();
        loaded_paths.dedup();
        Ok((theme, loaded_paths))
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::base("catppuccin-mocha")
    }
}

#[derive(Debug)]
pub enum ThemeError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Parse {
        path: PathBuf,
        message: String,
    },
    UnsupportedSchemaVersion {
        path: PathBuf,
        found: u32,
        expected: u32,
    },
    UnknownTheme(String),
}

impl std::fmt::Display for ThemeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(
                    formatter,
                    "failed to read theme {}: {source}",
                    path.display()
                )
            }
            Self::Parse { path, message } => {
                write!(
                    formatter,
                    "failed to parse theme {}: {message}",
                    path.display()
                )
            }
            Self::UnsupportedSchemaVersion {
                path,
                found,
                expected,
            } => write!(
                formatter,
                "{} uses unsupported schema-version {}; expected {}",
                path.display(),
                found,
                expected
            ),
            Self::UnknownTheme(name) => write!(formatter, "unknown theme `{name}`"),
        }
    }
}

impl std::error::Error for ThemeError {}

fn load_theme_file(
    paths: &ConfigPaths,
    themes_dir: &Path,
    path: &Path,
    name: &str,
    loaded_paths: &mut Vec<PathBuf>,
) -> Result<Theme, ThemeError> {
    loaded_paths.push(path.to_path_buf());
    let text = fs::read_to_string(path).map_err(|source| ThemeError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let parsed = parse_theme(name, &text).map_err(|message| ThemeError::Parse {
        path: path.to_path_buf(),
        message,
    })?;

    if parsed.schema_version != THEME_SCHEMA_VERSION {
        return Err(ThemeError::UnsupportedSchemaVersion {
            path: path.to_path_buf(),
            found: parsed.schema_version,
            expected: THEME_SCHEMA_VERSION,
        });
    }

    if let Some(base_name) = parsed.extends.as_deref() {
        let mut base = if let Some(theme) = Theme::builtin(base_name) {
            theme
        } else {
            let parent_scope = path.parent().unwrap_or(&paths.root);
            let parent_path = resolve_theme_reference_from(parent_scope, themes_dir, base_name)
                .ok_or_else(|| ThemeError::UnknownTheme(base_name.to_string()))?;
            load_theme_file(paths, themes_dir, &parent_path, base_name, loaded_paths)?
        };
        base.schema_version = parsed.schema_version;
        apply_sections(&mut base, &parsed.sections, path)?;
        base.name = name.to_string();
        return Ok(base);
    }

    let mut theme = Theme::base(name);
    theme.schema_version = parsed.schema_version;
    apply_sections(&mut theme, &parsed.sections, path)?;
    Ok(theme)
}

struct ParsedTheme {
    schema_version: u32,
    extends: Option<String>,
    sections: BTreeMap<String, BTreeMap<String, String>>,
}

fn parse_theme(name: &str, source: &str) -> Result<ParsedTheme, String> {
    let mut sections = BTreeMap::new();
    let mut current_section: Option<String> = None;
    let mut schema_version = THEME_SCHEMA_VERSION;
    let mut extends = None;

    for (line_no, raw_line) in source.lines().enumerate() {
        let line = strip_comments(raw_line).trim();
        if line.is_empty() {
            continue;
        }

        if current_section.is_none() {
            if let Some(section_name) = line.strip_suffix('{') {
                current_section = Some(section_name.trim().to_string());
                continue;
            }
            return Err(format!("{name}:{line_no}: expected `section {{`"));
        }

        if line == "}" {
            current_section = None;
            continue;
        }

        let Some((key, raw_value)) = line.split_once(':') else {
            return Err(format!("{name}:{line_no}: expected `key: value;`"));
        };
        let value = raw_value
            .trim()
            .strip_suffix(';')
            .ok_or_else(|| format!("{name}:{line_no}: missing trailing `;`"))?
            .trim()
            .trim_matches('"')
            .to_string();
        let section = current_section.as_ref().expect("active section");
        if section == "meta" {
            match key.trim() {
                "schema-version" => {
                    schema_version = value
                        .parse()
                        .map_err(|_| format!("{name}:{line_no}: invalid schema-version"))?;
                }
                "extends" => extends = Some(value),
                other => return Err(format!("{name}:{line_no}: unknown meta property `{other}`")),
            }
            continue;
        }
        sections
            .entry(section.clone())
            .or_insert_with(BTreeMap::new)
            .insert(key.trim().to_string(), value);
    }

    if current_section.is_some() {
        return Err(format!("{name}: unterminated section"));
    }

    Ok(ParsedTheme {
        schema_version,
        extends,
        sections,
    })
}

fn strip_comments(line: &str) -> &str {
    let slash = line.find("//");
    match slash {
        Some(index) => &line[..index],
        None => line,
    }
}

impl Theme {
    fn base(name: &str) -> Self {
        Self {
            name: name.to_string(),
            schema_version: THEME_SCHEMA_VERSION,
            window_background: Color::rgb(30, 30, 46),
            panel_padding: 16,
            item_spacing_x: 8.0,
            item_spacing_y: 6.0,
            header_gap: 12.0,
            row_gap: 2.0,
            header_height: 54.0,
            badge_width: 96.0,
            badge_background: Color::rgb(49, 50, 68),
            badge_foreground: Color::rgb(137, 180, 250),
            badge_radius: 8,
            badge_padding_x: 12,
            badge_padding_y: 8,
            badge_font_size: 21.0,
            input_background: Color::rgb(17, 17, 27),
            input_foreground: Color::rgb(205, 214, 244),
            input_placeholder_foreground: Color::rgb(108, 112, 134),
            input_padding_x: 14,
            input_padding_y: 8,
            input_font_size: 20.0,
            row_background: Color::rgb(30, 30, 46),
            row_hover_background: Color::rgb(49, 50, 68),
            row_selected_background: Color::rgb(69, 71, 90),
            row_foreground: Color::rgb(239, 241, 245),
            row_secondary_foreground: Color::rgb(166, 173, 200),
            row_primary_font_size: 19.0,
            row_secondary_font_size: 14.0,
            row_height: 40.0,
            row_padding_x: 14.0,
            row_padding_y: 8.0,
            empty_foreground: Color::rgb(166, 173, 200),
            empty_font_size: 16.0,
            error_foreground: Color::rgb(243, 139, 168),
            heading_font_size: 22.0,
            body_font_size: 16.0,
            button_font_size: 16.0,
            small_font_size: 12.0,
        }
    }
}

fn apply_sections(
    theme: &mut Theme,
    sections: &BTreeMap<String, BTreeMap<String, String>>,
    path: &Path,
) -> Result<(), ThemeError> {
    for (section, values) in sections {
        match section.as_str() {
            "window" => apply_window(theme, values, path)?,
            "spacing" => apply_spacing(theme, values, path)?,
            "mode-badge" => apply_badge(theme, values, path)?,
            "input" => apply_input(theme, values, path)?,
            "result-row" => apply_result_row(theme, values, path)?,
            "status" => apply_status(theme, values, path)?,
            "egui" => apply_egui(theme, values, path)?,
            other => {
                return Err(ThemeError::Parse {
                    path: path.to_path_buf(),
                    message: format!("unknown section `{other}`"),
                });
            }
        }
    }
    Ok(())
}

fn apply_window(
    theme: &mut Theme,
    values: &BTreeMap<String, String>,
    path: &Path,
) -> Result<(), ThemeError> {
    for (key, value) in values {
        match key.as_str() {
            "background" => theme.window_background = parse_color(value, path, key)?,
            "panel-padding" => theme.panel_padding = parse_i8(value, path, key)?,
            other => return unknown_property(path, "window", other),
        }
    }
    Ok(())
}

fn apply_spacing(
    theme: &mut Theme,
    values: &BTreeMap<String, String>,
    path: &Path,
) -> Result<(), ThemeError> {
    for (key, value) in values {
        match key.as_str() {
            "item-x" => theme.item_spacing_x = parse_f32(value, path, key)?,
            "item-y" => theme.item_spacing_y = parse_f32(value, path, key)?,
            "header-gap" => theme.header_gap = parse_f32(value, path, key)?,
            "row-gap" => theme.row_gap = parse_f32(value, path, key)?,
            other => return unknown_property(path, "spacing", other),
        }
    }
    Ok(())
}

fn apply_badge(
    theme: &mut Theme,
    values: &BTreeMap<String, String>,
    path: &Path,
) -> Result<(), ThemeError> {
    for (key, value) in values {
        match key.as_str() {
            "width" => theme.badge_width = parse_f32(value, path, key)?,
            "height" => theme.header_height = parse_f32(value, path, key)?,
            "background" => theme.badge_background = parse_color(value, path, key)?,
            "foreground" => theme.badge_foreground = parse_color(value, path, key)?,
            "radius" => theme.badge_radius = parse_u8(value, path, key)?,
            "padding-x" => theme.badge_padding_x = parse_i8(value, path, key)?,
            "padding-y" => theme.badge_padding_y = parse_i8(value, path, key)?,
            "font-size" => theme.badge_font_size = parse_f32(value, path, key)?,
            other => return unknown_property(path, "mode-badge", other),
        }
    }
    Ok(())
}

fn apply_input(
    theme: &mut Theme,
    values: &BTreeMap<String, String>,
    path: &Path,
) -> Result<(), ThemeError> {
    for (key, value) in values {
        match key.as_str() {
            "background" => theme.input_background = parse_color(value, path, key)?,
            "foreground" => theme.input_foreground = parse_color(value, path, key)?,
            "placeholder-foreground" => {
                theme.input_placeholder_foreground = parse_color(value, path, key)?
            }
            "padding-x" => theme.input_padding_x = parse_i8(value, path, key)?,
            "padding-y" => theme.input_padding_y = parse_i8(value, path, key)?,
            "font-size" => theme.input_font_size = parse_f32(value, path, key)?,
            other => return unknown_property(path, "input", other),
        }
    }
    Ok(())
}

fn apply_result_row(
    theme: &mut Theme,
    values: &BTreeMap<String, String>,
    path: &Path,
) -> Result<(), ThemeError> {
    for (key, value) in values {
        match key.as_str() {
            "height" => theme.row_height = parse_f32(value, path, key)?,
            "background" => theme.row_background = parse_color(value, path, key)?,
            "hover-background" => theme.row_hover_background = parse_color(value, path, key)?,
            "selected-background" => theme.row_selected_background = parse_color(value, path, key)?,
            "foreground" => theme.row_foreground = parse_color(value, path, key)?,
            "secondary-foreground" => {
                theme.row_secondary_foreground = parse_color(value, path, key)?
            }
            "primary-font-size" => theme.row_primary_font_size = parse_f32(value, path, key)?,
            "secondary-font-size" => theme.row_secondary_font_size = parse_f32(value, path, key)?,
            "padding-x" => theme.row_padding_x = parse_f32(value, path, key)?,
            "padding-y" => theme.row_padding_y = parse_f32(value, path, key)?,
            other => return unknown_property(path, "result-row", other),
        }
    }
    Ok(())
}

fn apply_status(
    theme: &mut Theme,
    values: &BTreeMap<String, String>,
    path: &Path,
) -> Result<(), ThemeError> {
    for (key, value) in values {
        match key.as_str() {
            "empty-foreground" => theme.empty_foreground = parse_color(value, path, key)?,
            "empty-font-size" => theme.empty_font_size = parse_f32(value, path, key)?,
            "error-foreground" => theme.error_foreground = parse_color(value, path, key)?,
            other => return unknown_property(path, "status", other),
        }
    }
    Ok(())
}

fn apply_egui(
    theme: &mut Theme,
    values: &BTreeMap<String, String>,
    path: &Path,
) -> Result<(), ThemeError> {
    for (key, value) in values {
        match key.as_str() {
            "heading-font-size" => theme.heading_font_size = parse_f32(value, path, key)?,
            "body-font-size" => theme.body_font_size = parse_f32(value, path, key)?,
            "button-font-size" => theme.button_font_size = parse_f32(value, path, key)?,
            "small-font-size" => theme.small_font_size = parse_f32(value, path, key)?,
            other => return unknown_property(path, "egui", other),
        }
    }
    Ok(())
}

fn parse_color(value: &str, path: &Path, key: &str) -> Result<Color, ThemeError> {
    let hex = value.trim().trim_start_matches('#');
    let bytes = match hex.len() {
        6 => u32::from_str_radix(hex, 16).ok().map(|raw| (raw, 255)),
        8 => u32::from_str_radix(hex, 16)
            .ok()
            .map(|raw| (raw >> 8, (raw & 0xff) as u8)),
        _ => None,
    }
    .ok_or_else(|| ThemeError::Parse {
        path: path.to_path_buf(),
        message: format!("invalid color for `{key}`: `{value}`"),
    })?;
    let raw = bytes.0;
    Ok(Color {
        r: ((raw >> 16) & 0xff) as u8,
        g: ((raw >> 8) & 0xff) as u8,
        b: (raw & 0xff) as u8,
        a: bytes.1,
    })
}

fn parse_f32(value: &str, path: &Path, key: &str) -> Result<f32, ThemeError> {
    value.parse().map_err(|_| ThemeError::Parse {
        path: path.to_path_buf(),
        message: format!("invalid numeric value for `{key}`: `{value}`"),
    })
}

fn parse_i8(value: &str, path: &Path, key: &str) -> Result<i8, ThemeError> {
    value.parse().map_err(|_| ThemeError::Parse {
        path: path.to_path_buf(),
        message: format!("invalid integer value for `{key}`: `{value}`"),
    })
}

fn parse_u8(value: &str, path: &Path, key: &str) -> Result<u8, ThemeError> {
    value.parse().map_err(|_| ThemeError::Parse {
        path: path.to_path_buf(),
        message: format!("invalid integer value for `{key}`: `{value}`"),
    })
}

fn unknown_property<T>(path: &Path, section: &str, key: &str) -> Result<T, ThemeError> {
    Err(ThemeError::Parse {
        path: path.to_path_buf(),
        message: format!("unknown property `{key}` in section `{section}`"),
    })
}

fn builtin_theme_source(name: &str) -> Option<&'static str> {
    match name {
        "catppuccin-mocha" => Some(CATPPUCCIN_MOCHA),
        "gruvbox-dark" => Some(GRUVBOX_DARK),
        _ => None,
    }
}

fn resolve_theme_reference(themes_dir: &Path, reference: &str) -> Option<PathBuf> {
    resolve_theme_reference_from(themes_dir, themes_dir, reference)
}

fn resolve_theme_reference_from(
    scope_dir: &Path,
    themes_dir: &Path,
    reference: &str,
) -> Option<PathBuf> {
    let direct = PathBuf::from(reference);
    if direct.is_absolute() {
        return resolve_theme_path_candidates(&direct);
    }

    if let Some(path) = resolve_theme_path_candidates(&scope_dir.join(reference)) {
        return Some(path);
    }
    if let Some(path) = resolve_theme_path_candidates(&themes_dir.join(reference)) {
        return Some(path);
    }

    find_theme_in_tree(themes_dir, reference)
}

fn resolve_theme_path_candidates(base: &Path) -> Option<PathBuf> {
    if base.is_file() {
        return Some(base.to_path_buf());
    }

    if base.extension().is_none() {
        let themed = base.with_extension("theme");
        if themed.is_file() {
            return Some(themed);
        }
    }

    if base.is_dir() {
        for candidate in theme_entry_candidates(base) {
            if candidate.is_file() {
                return Some(candidate);
            }
        }

        let mut nested = Vec::new();
        collect_theme_files(base, &mut nested);
        nested.sort();
        if let Some(first) = nested.into_iter().next() {
            return Some(first);
        }
    }

    None
}

fn theme_entry_candidates(directory: &Path) -> [PathBuf; 4] {
    let dirname = directory
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("theme");
    [
        directory.join("vega.theme"),
        directory.join("theme.theme"),
        directory.join("index.theme"),
        directory.join(format!("{dirname}.theme")),
    ]
}

fn find_theme_in_tree(themes_dir: &Path, reference: &str) -> Option<PathBuf> {
    let normalized_reference = normalize_theme_name(reference);
    let mut files = Vec::new();
    collect_theme_files(themes_dir, &mut files);
    files.sort();

    files.into_iter().find(|file| {
        theme_aliases(themes_dir, file)
            .into_iter()
            .any(|alias| alias == normalized_reference)
    })
}

fn collect_theme_files(directory: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_theme_files(&path, files);
            continue;
        }
        if path.extension().and_then(|value| value.to_str()) == Some("theme") {
            files.push(path);
        }
    }
}

fn theme_aliases(themes_dir: &Path, file: &Path) -> Vec<String> {
    let mut aliases = Vec::new();
    let Ok(relative) = file.strip_prefix(themes_dir) else {
        return aliases;
    };

    aliases.push(normalize_theme_name(&relative.to_string_lossy()));

    let stem = file
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    if !stem.is_empty() {
        aliases.push(normalize_theme_name(stem));
    }

    if let Some(stripped) = relative.to_string_lossy().strip_suffix(".theme") {
        aliases.push(normalize_theme_name(stripped));
    }

    if matches!(stem, "vega" | "theme" | "index")
        && let Some(parent) = relative.parent()
        && !parent.as_os_str().is_empty()
    {
        aliases.push(normalize_theme_name(&parent.to_string_lossy()));
    }

    aliases.sort();
    aliases.dedup();
    aliases
}

fn normalize_theme_name(value: &str) -> String {
    value
        .trim()
        .trim_end_matches(".theme")
        .replace('\\', "/")
        .trim_matches('/')
        .to_ascii_lowercase()
}

const CATPPUCCIN_MOCHA: &str = r#"
meta {
  schema-version: 1;
}

window {
  background: #1e1e2e;
  panel-padding: 16;
}

spacing {
  item-x: 8;
  item-y: 6;
  header-gap: 12;
  row-gap: 2;
}

mode-badge {
  width: 96;
  height: 54;
  background: #313244;
  foreground: #89b4fa;
  radius: 8;
  padding-x: 12;
  padding-y: 8;
  font-size: 21;
}

input {
  background: #11111b;
  foreground: #cdd6f4;
  placeholder-foreground: #6c7086;
  padding-x: 14;
  padding-y: 8;
  font-size: 20;
}

result-row {
  height: 40;
  background: #1e1e2e;
  hover-background: #313244;
  selected-background: #45475a;
  foreground: #eff1f5;
  secondary-foreground: #a6adc8;
  primary-font-size: 19;
  secondary-font-size: 14;
  padding-x: 14;
  padding-y: 8;
}

status {
  empty-foreground: #a6adc8;
  empty-font-size: 16;
  error-foreground: #f38ba8;
}

egui {
  heading-font-size: 22;
  body-font-size: 16;
  button-font-size: 16;
  small-font-size: 12;
}
"#;

const GRUVBOX_DARK: &str = r#"
meta {
  schema-version: 1;
}

window {
  background: #282828;
  panel-padding: 16;
}

spacing {
  item-x: 8;
  item-y: 6;
  header-gap: 12;
  row-gap: 2;
}

mode-badge {
  width: 96;
  height: 54;
  background: #3c3836;
  foreground: #fabd2f;
  radius: 8;
  padding-x: 12;
  padding-y: 8;
  font-size: 21;
}

input {
  background: #1d2021;
  foreground: #ebdbb2;
  placeholder-foreground: #928374;
  padding-x: 14;
  padding-y: 8;
  font-size: 20;
}

result-row {
  height: 40;
  background: #282828;
  hover-background: #3c3836;
  selected-background: #504945;
  foreground: #ebdbb2;
  secondary-foreground: #a89984;
  primary-font-size: 19;
  secondary-font-size: 14;
  padding-x: 14;
  padding-y: 8;
}

status {
  empty-foreground: #a89984;
  empty-font-size: 16;
  error-foreground: #fb4934;
}

egui {
  heading-font-size: 22;
  body-font-size: 16;
  button-font-size: 16;
  small-font-size: 12;
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_builtin_theme() {
        let theme = Theme::builtin("catppuccin-mocha").expect("theme");
        assert_eq!(theme.badge_width, 96.0);
        assert_eq!(theme.row_height, 40.0);
    }

    #[test]
    fn parses_extends_meta() {
        let parsed = parse_theme(
            "demo",
            r#"
            meta {
              schema-version: 1;
              extends: "gruvbox-dark";
            }

            result-row {
              selected-background: #000000;
            }
            "#,
        )
        .expect("parsed");

        assert_eq!(parsed.extends.as_deref(), Some("gruvbox-dark"));
        assert_eq!(
            parsed
                .sections
                .get("result-row")
                .and_then(|section| section.get("selected-background"))
                .map(String::as_str),
            Some("#000000")
        );
    }

    #[test]
    fn resolves_repo_style_theme_directory() {
        let base = Path::new("/tmp/themes");
        let theme = resolve_theme_reference_from(base, base, "catppuccin");
        assert!(theme.is_none());

        let aliases = theme_aliases(
            Path::new("/tmp/themes"),
            Path::new("/tmp/themes/catppuccin/vega.theme"),
        );
        assert!(aliases.contains(&"catppuccin".to_string()));
        assert!(aliases.contains(&"catppuccin/vega".to_string()));
    }
}
