use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::candidate::{Candidate, CandidateAction};

pub trait Mode {
    fn name(&self) -> &'static str;
    fn load(&self) -> Result<Vec<Candidate>, ModeError>;
    fn execute(&self, candidate: &Candidate) -> Result<(), ModeError>;
}

#[derive(Debug)]
pub struct DmenuMode {
    input: String,
}

impl DmenuMode {
    pub fn new(input: String) -> Self {
        Self { input }
    }
}

impl Mode for DmenuMode {
    fn name(&self) -> &'static str {
        "dmenu"
    }

    fn load(&self) -> Result<Vec<Candidate>, ModeError> {
        Ok(self
            .input
            .lines()
            .enumerate()
            .filter_map(|(index, line)| {
                let trimmed = line.trim_end();
                (!trimmed.is_empty()).then(|| Candidate::new(format!("stdin:{index}"), trimmed))
            })
            .collect())
    }

    fn execute(&self, candidate: &Candidate) -> Result<(), ModeError> {
        println!("{}", candidate.primary);
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct RunMode;

impl RunMode {
    pub fn new() -> Self {
        Self
    }
}

impl Mode for RunMode {
    fn name(&self) -> &'static str {
        "run"
    }

    fn load(&self) -> Result<Vec<Candidate>, ModeError> {
        let mut seen = BTreeSet::new();
        let mut candidates = Vec::new();
        let path = env::var_os("PATH").unwrap_or_default();

        for directory in env::split_paths(&path) {
            let Ok(entries) = fs::read_dir(&directory) else {
                continue;
            };

            for entry in entries.flatten() {
                let path = entry.path();
                let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
                    continue;
                };
                if !seen.insert(name.to_string()) || !is_executable_file(&path) {
                    continue;
                }

                candidates.push(
                    Candidate::new(format!("run:{name}"), name)
                        .with_secondary(path.display().to_string())
                        .with_action(CandidateAction::Exec(vec![path.display().to_string()])),
                );
            }
        }

        Ok(candidates)
    }

    fn execute(&self, candidate: &Candidate) -> Result<(), ModeError> {
        match &candidate.action {
            CandidateAction::Exec(argv) if !argv.is_empty() => {
                Command::new(&argv[0]).args(&argv[1..]).spawn()?;
                Ok(())
            }
            _ => Err(ModeError::Execution(format!(
                "candidate `{}` has no executable action",
                candidate.primary
            ))),
        }
    }
}

#[derive(Debug, Default)]
pub struct DesktopMode;

impl DesktopMode {
    pub fn new() -> Self {
        Self
    }
}

impl Mode for DesktopMode {
    fn name(&self) -> &'static str {
        "drun"
    }

    fn load(&self) -> Result<Vec<Candidate>, ModeError> {
        let mut entries = BTreeMap::new();
        for directory in desktop_data_dirs() {
            let applications = directory.join("applications");
            collect_desktop_entries(&applications, &mut entries)?;
        }

        let mut candidates = Vec::new();
        for (id, path) in entries {
            let Some(entry) = parse_desktop_file(&path)? else {
                continue;
            };
            if let Some(candidate) = desktop_entry_candidate(&id, entry) {
                candidates.push(candidate);
            }
        }

        Ok(candidates)
    }

    fn execute(&self, candidate: &Candidate) -> Result<(), ModeError> {
        match &candidate.action {
            CandidateAction::DesktopExec(exec) => {
                let argv = parse_desktop_exec(exec)?;
                if argv.is_empty() {
                    return Err(ModeError::Execution("empty desktop Exec command".into()));
                }
                Command::new(&argv[0]).args(&argv[1..]).spawn()?;
                Ok(())
            }
            _ => Err(ModeError::Execution(format!(
                "candidate `{}` has no desktop Exec action",
                candidate.primary
            ))),
        }
    }
}

#[cfg(unix)]
fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable_file(path: &Path) -> bool {
    path.is_file()
}

fn desktop_data_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(home) = env::var_os("HOME") {
        dirs.push(PathBuf::from(home).join(".local/share"));
    }

    if let Some(value) = env::var_os("XDG_DATA_DIRS") {
        dirs.extend(env::split_paths(&value));
    } else {
        dirs.push(PathBuf::from("/usr/local/share"));
        dirs.push(PathBuf::from("/usr/share"));
    }

    dirs
}

fn collect_desktop_entries(
    directory: &Path,
    entries: &mut BTreeMap<String, PathBuf>,
) -> Result<(), ModeError> {
    let Ok(read_dir) = fs::read_dir(directory) else {
        return Ok(());
    };

    for entry in read_dir.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_desktop_entries(&path, entries)?;
            continue;
        }
        if path.extension().and_then(|value| value.to_str()) != Some("desktop") {
            continue;
        }
        let Some(id) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        entries.entry(id.to_string()).or_insert(path);
    }

    Ok(())
}

#[derive(Debug, Default)]
struct DesktopEntry {
    name: String,
    generic_name: Option<String>,
    comment: Option<String>,
    exec: Option<String>,
    no_display: bool,
    hidden: bool,
}

fn parse_desktop_file(path: &Path) -> Result<Option<DesktopEntry>, ModeError> {
    let content = fs::read_to_string(path)?;
    let mut in_desktop_entry = false;
    let mut entry = DesktopEntry::default();

    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            in_desktop_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_desktop_entry {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key {
            "Name" => entry.name = value.to_string(),
            "GenericName" => entry.generic_name = Some(value.to_string()),
            "Comment" => entry.comment = Some(value.to_string()),
            "Exec" => entry.exec = Some(value.to_string()),
            "NoDisplay" => entry.no_display = value.eq_ignore_ascii_case("true"),
            "Hidden" => entry.hidden = value.eq_ignore_ascii_case("true"),
            _ => {}
        }
    }

    Ok((!entry.name.is_empty()).then_some(entry))
}

fn desktop_entry_candidate(id: &str, entry: DesktopEntry) -> Option<Candidate> {
    if entry.no_display || entry.hidden || entry.name.is_empty() {
        return None;
    }

    let mut candidate = Candidate::new(format!("desktop:{id}"), entry.name)
        .with_secondary_display_only(entry.generic_name.unwrap_or_default());
    if let Some(exec) = entry.exec {
        candidate = candidate.with_action(CandidateAction::DesktopExec(exec));
    }

    Some(candidate)
}

fn parse_desktop_exec(exec: &str) -> Result<Vec<String>, ModeError> {
    let mut argv = shell_words(exec)?;
    argv.retain(|part| !part.starts_with('%'));
    if let Some(program) = argv.first() {
        reject_shell_interpreter(program)?;
    }
    Ok(argv)
}

fn reject_shell_interpreter(program: &str) -> Result<(), ModeError> {
    let name = Path::new(program)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(program);
    if matches!(
        name,
        "sh" | "bash" | "dash" | "zsh" | "fish" | "csh" | "tcsh"
    ) {
        return Err(ModeError::Execution(format!(
            "desktop Exec starts shell interpreter `{name}`, which is unsupported"
        )));
    }
    Ok(())
}

fn shell_words(input: &str) -> Result<Vec<String>, ModeError> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut chars = input.chars().peekable();
    let mut quote = None;

    while let Some(ch) = chars.next() {
        match (quote, ch) {
            (None, '\'') => quote = Some('\''),
            (None, '"') => quote = Some('"'),
            (Some('\''), '\'') | (Some('"'), '"') => quote = None,
            (None, '\\') | (Some('"'), '\\') => {
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            }
            (None, ch) if ch.is_whitespace() => {
                if !current.is_empty() {
                    words.push(std::mem::take(&mut current));
                }
            }
            (None, ch) if matches!(ch, '|' | '&' | ';' | '<' | '>' | '`') => {
                return Err(ModeError::Execution(format!(
                    "desktop Exec contains unsupported shell operator `{ch}`"
                )));
            }
            (_, ch) => current.push(ch),
        }
    }

    if quote.is_some() {
        return Err(ModeError::Execution(
            "unterminated quote in desktop Exec".into(),
        ));
    }
    if !current.is_empty() {
        words.push(current);
    }

    Ok(words)
}

#[derive(Debug)]
pub enum ModeError {
    Io(io::Error),
    Execution(String),
}

impl std::fmt::Display for ModeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "{error}"),
            Self::Execution(message) => write!(formatter, "{message}"),
        }
    }
}

impl std::error::Error for ModeError {}

impl From<io::Error> for ModeError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_candidates_match_only_name_and_generic_name() {
        let candidate = desktop_entry_candidate(
            "alarm.desktop",
            DesktopEntry {
                name: "KAlarm".to_string(),
                generic_name: Some("Personal Alarm Scheduler".to_string()),
                comment: Some("Set alarms and timers".to_string()),
                exec: Some("kalarm".to_string()),
                no_display: false,
                hidden: false,
            },
        )
        .unwrap();

        assert_eq!(candidate.searchable, vec!["KAlarm".to_string()]);
        assert_eq!(
            candidate.secondary,
            Some("Personal Alarm Scheduler".to_string())
        );
    }

    #[test]
    fn parses_desktop_exec_without_shell() {
        let argv = parse_desktop_exec("firefox --new-window %u").unwrap();
        assert_eq!(argv, vec!["firefox", "--new-window"]);
    }

    #[test]
    fn rejects_shell_operators_in_desktop_exec() {
        assert!(parse_desktop_exec("firefox; rm -rf /").is_err());
    }

    #[test]
    fn rejects_shell_interpreters_in_desktop_exec() {
        assert!(parse_desktop_exec("sh -c 'x; y'").is_err());
        assert!(parse_desktop_exec("/bin/bash -lc firefox").is_err());
    }
}
