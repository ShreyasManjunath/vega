use std::collections::BTreeMap;
use std::env;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use crate::candidate::{Candidate, CandidateId};

#[derive(Clone, Debug)]
pub struct FzfConfig {
    pub binary: String,
    pub timeout: Duration,
    pub extra_flags: Vec<String>,
}

impl Default for FzfConfig {
    fn default() -> Self {
        Self {
            binary: "fzf".to_string(),
            timeout: Duration::from_millis(1500),
            extra_flags: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub struct FzfBackend {
    config: FzfConfig,
}

#[derive(Clone, Debug, Default)]
pub struct QueryCancellation {
    cancelled: Arc<AtomicBool>,
}

impl QueryCancellation {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

impl FzfBackend {
    pub fn start(config: FzfConfig) -> Result<Self, FzfError> {
        if config.binary.trim().is_empty() {
            return Err(FzfError::InvalidConfig("fzf binary cannot be empty".into()));
        }
        Ok(Self { config })
    }

    pub fn query(&self, request: QueryRequest) -> Result<QueryResponse, FzfError> {
        self.query_with_cancellation(request, None)
    }

    pub fn query_with_cancellation(
        &self,
        request: QueryRequest,
        cancellation: Option<&QueryCancellation>,
    ) -> Result<QueryResponse, FzfError> {
        let started = Instant::now();
        let candidate_count = request.candidates.len();
        let transport = CandidateTransport::new(request.candidates)?;
        if let Some(matches) = transport.exact_matches(&request.query, request.limit) {
            return Ok(QueryResponse {
                candidate_count,
                matches,
                elapsed: started.elapsed(),
                stderr: String::new(),
            });
        }
        let input = transport.serialize();
        let mut child = self.spawn(&request.query)?;
        let mut stdin = child.stdin.take().ok_or(FzfError::MissingPipe("stdin"))?;
        let stdout = child.stdout.take().ok_or(FzfError::MissingPipe("stdout"))?;
        let stderr = child.stderr.take().ok_or(FzfError::MissingPipe("stderr"))?;

        let stdout_reader = read_pipe(stdout);
        let stderr_reader = read_pipe(stderr);

        let stdin_writer = thread::spawn(move || {
            let result = stdin
                .write_all(input.as_bytes())
                .and_then(|_| stdin.flush());
            drop(stdin);
            result
        });

        let status = wait_with_timeout(&mut child, self.config.timeout, cancellation)?;
        stdin_writer
            .join()
            .map_err(|_| FzfError::WorkerPanicked)??;
        let stdout = stdout_reader
            .join()
            .map_err(|_| FzfError::WorkerPanicked)??;
        let stderr = stderr_reader
            .join()
            .map_err(|_| FzfError::WorkerPanicked)??;

        if !status.success() && status.code() != Some(1) {
            return Err(FzfError::ProcessFailed {
                status,
                stderr: String::from_utf8_lossy(&stderr).trim().to_string(),
            });
        }

        let output = String::from_utf8(stdout).map_err(|_| FzfError::InvalidUtf8)?;
        let mut matches = Vec::new();
        for line in output.lines().take(request.limit) {
            let candidate = transport.parse_selected_line(line)?;
            matches.push(FzfMatch {
                candidate,
                raw_line: line.to_string(),
            });
        }

        Ok(QueryResponse {
            candidate_count,
            matches,
            elapsed: started.elapsed(),
            stderr: String::from_utf8_lossy(&stderr).trim().to_string(),
        })
    }

    pub fn shutdown(self) {}

    fn spawn(&self, query: &str) -> Result<Child, FzfError> {
        let mut command = Command::new(&self.config.binary);
        command
            .arg("--filter")
            .arg(query)
            .arg("--delimiter")
            .arg("\t")
            .arg("--with-nth")
            .arg("2..")
            .arg("--nth")
            .arg("2..")
            .arg("--prompt")
            .arg("vega> ")
            .args(&self.config.extra_flags)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        command.spawn().map_err(|error| match error.kind() {
            io::ErrorKind::NotFound => FzfError::BinaryNotFound(self.config.binary.clone()),
            _ => FzfError::SpawnFailed(error),
        })
    }
}

pub fn resolve_binary_path(binary: &str) -> Option<PathBuf> {
    if binary.is_empty() {
        return None;
    }

    let binary_path = Path::new(binary);
    if binary_path.components().count() > 1 {
        return binary_path.is_file().then(|| binary_path.to_path_buf());
    }

    let path = env::var_os("PATH")?;
    env::split_paths(&path)
        .map(|dir| dir.join(binary))
        .find(|candidate| candidate.is_file())
}

#[derive(Debug)]
pub struct QueryRequest {
    pub query: String,
    pub candidates: Vec<Candidate>,
    pub limit: usize,
}

#[derive(Debug)]
pub struct QueryResponse {
    pub candidate_count: usize,
    pub matches: Vec<FzfMatch>,
    pub elapsed: Duration,
    pub stderr: String,
}

#[derive(Debug)]
pub struct FzfMatch {
    pub candidate: Candidate,
    pub raw_line: String,
}

#[derive(Debug)]
struct CandidateTransport {
    candidates: BTreeMap<CandidateId, Candidate>,
    lines: Vec<String>,
}

impl CandidateTransport {
    fn new(candidates: Vec<Candidate>) -> Result<Self, FzfError> {
        let mut mapped = BTreeMap::new();
        let mut lines = Vec::with_capacity(candidates.len());

        for candidate in candidates {
            validate_field(candidate.id.as_str(), "id")?;
            validate_field(&candidate.primary, "primary")?;
            if mapped.contains_key(&candidate.id) {
                return Err(FzfError::DuplicateId(candidate.id.to_string()));
            }

            let searchable = if candidate.searchable.is_empty() {
                candidate.primary.clone()
            } else {
                candidate.searchable.join(" ")
            };
            validate_field(&searchable, "searchable")?;

            let secondary = candidate.secondary.clone().unwrap_or_default();
            validate_field(&secondary, "secondary")?;
            lines.push(format!(
                "{}\t{}\t{}\t{}",
                candidate.id, candidate.primary, secondary, searchable
            ));
            mapped.insert(candidate.id.clone(), candidate);
        }

        Ok(Self {
            candidates: mapped,
            lines,
        })
    }

    fn serialize(&self) -> String {
        let mut output = String::new();
        for line in &self.lines {
            output.push_str(line);
            output.push('\n');
        }
        output
    }

    fn exact_matches(&self, query: &str, limit: usize) -> Option<Vec<FzfMatch>> {
        let needle = normalize(query);
        if needle.is_empty() {
            return None;
        }

        let mut ranked = Vec::new();
        for candidate in self.candidates.values() {
            if let Some(rank) = exact_match_rank(candidate, &needle) {
                ranked.push((rank, candidate));
            }
        }

        if ranked.is_empty() {
            return None;
        }

        ranked.sort_by_key(|left| left.0);
        Some(
            ranked
                .into_iter()
                .take(limit)
                .map(|(_, candidate)| FzfMatch {
                    candidate: candidate.clone(),
                    raw_line: candidate.id.to_string(),
                })
                .collect(),
        )
    }

    fn parse_selected_line(&self, line: &str) -> Result<Candidate, FzfError> {
        let id = line
            .split('\t')
            .next()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| FzfError::InvalidOutput(line.to_string()))?;
        self.candidates
            .get(&CandidateId::from(id))
            .cloned()
            .ok_or_else(|| FzfError::UnknownCandidateId(id.to_string()))
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum ExactMatchKind {
    PrimaryExact,
    SecondaryExact,
    PrimaryPrefix,
    SecondaryPrefix,
    PrimarySubstring,
    SecondarySubstring,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ExactMatchRank {
    kind: ExactMatchKind,
    field_len: usize,
    primary_len: usize,
}

fn exact_match_rank(candidate: &Candidate, needle: &str) -> Option<ExactMatchRank> {
    let primary_len = normalize(&candidate.primary).chars().count();
    [
        match_field(&candidate.primary, needle, true, primary_len),
        candidate
            .secondary
            .as_deref()
            .and_then(|secondary| match_field(secondary, needle, false, primary_len)),
    ]
    .into_iter()
    .flatten()
    .min_by_key(|rank| *rank)
}

fn match_field(
    field: &str,
    needle: &str,
    primary: bool,
    primary_len: usize,
) -> Option<ExactMatchRank> {
    let haystack = normalize(field);
    if haystack.is_empty() {
        return None;
    }

    let kind = if haystack == needle {
        if primary {
            ExactMatchKind::PrimaryExact
        } else {
            ExactMatchKind::SecondaryExact
        }
    } else if haystack.starts_with(needle) {
        if primary {
            ExactMatchKind::PrimaryPrefix
        } else {
            ExactMatchKind::SecondaryPrefix
        }
    } else if haystack.contains(needle) {
        if primary {
            ExactMatchKind::PrimarySubstring
        } else {
            ExactMatchKind::SecondarySubstring
        }
    } else {
        return None;
    };

    Some(ExactMatchRank {
        kind,
        field_len: haystack.chars().count(),
        primary_len,
    })
}

fn normalize(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn validate_field(value: &str, field: &'static str) -> Result<(), FzfError> {
    if value.contains('\n') || value.contains('\t') || value.contains('\0') {
        return Err(FzfError::InvalidCandidateField(field));
    }
    Ok(())
}

fn read_pipe<R>(mut reader: R) -> thread::JoinHandle<io::Result<Vec<u8>>>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut output = Vec::new();
        reader.read_to_end(&mut output)?;
        Ok(output)
    })
}

fn wait_with_timeout(
    child: &mut Child,
    timeout: Duration,
    cancellation: Option<&QueryCancellation>,
) -> Result<ExitStatus, FzfError> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }

        if cancellation.is_some_and(QueryCancellation::is_cancelled) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(FzfError::Cancelled);
        }

        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(FzfError::Timeout(timeout));
        }

        thread::sleep(Duration::from_millis(5));
    }
}

#[derive(Debug)]
pub enum FzfError {
    InvalidConfig(String),
    BinaryNotFound(String),
    SpawnFailed(io::Error),
    MissingPipe(&'static str),
    Io(io::Error),
    WorkerPanicked,
    Cancelled,
    Timeout(Duration),
    ProcessFailed { status: ExitStatus, stderr: String },
    InvalidUtf8,
    InvalidCandidateField(&'static str),
    DuplicateId(String),
    InvalidOutput(String),
    UnknownCandidateId(String),
}

impl std::fmt::Display for FzfError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidConfig(message) => write!(formatter, "invalid fzf config: {message}"),
            Self::BinaryNotFound(binary) => write!(formatter, "`{binary}` was not found in PATH"),
            Self::SpawnFailed(error) => write!(formatter, "failed to spawn fzf: {error}"),
            Self::MissingPipe(pipe) => write!(formatter, "fzf {pipe} pipe was unavailable"),
            Self::Io(error) => write!(formatter, "fzf I/O error: {error}"),
            Self::WorkerPanicked => write!(formatter, "fzf worker thread panicked"),
            Self::Cancelled => write!(formatter, "fzf query was cancelled"),
            Self::Timeout(timeout) => {
                write!(formatter, "fzf timed out after {}ms", timeout.as_millis())
            }
            Self::ProcessFailed { status, stderr } => {
                write!(formatter, "fzf exited with {status}")?;
                if !stderr.is_empty() {
                    write!(formatter, ": {stderr}")?;
                }
                Ok(())
            }
            Self::InvalidUtf8 => write!(formatter, "fzf emitted non-UTF-8 output"),
            Self::InvalidCandidateField(field) => {
                write!(
                    formatter,
                    "candidate {field} contains a tab, newline, or NUL byte"
                )
            }
            Self::DuplicateId(id) => write!(formatter, "duplicate candidate id `{id}`"),
            Self::InvalidOutput(line) => write!(formatter, "invalid fzf output line `{line}`"),
            Self::UnknownCandidateId(id) => {
                write!(formatter, "fzf returned unknown candidate id `{id}`")
            }
        }
    }
}

impl std::error::Error for FzfError {}

impl From<io::Error> for FzfError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::candidate::CandidateAction;

    #[test]
    fn transport_preserves_duplicate_visible_labels_with_unique_ids() {
        let first = Candidate::new("app:1", "Terminal");
        let second = Candidate::new("app:2", "Terminal");
        let transport = CandidateTransport::new(vec![first, second]).unwrap();

        assert_eq!(
            transport
                .parse_selected_line("app:2\tTerminal\t\tTerminal")
                .unwrap()
                .id
                .as_str(),
            "app:2"
        );
    }

    #[test]
    fn transport_rejects_duplicate_ids() {
        let candidates = vec![Candidate::new("same", "One"), Candidate::new("same", "Two")];
        assert!(matches!(
            CandidateTransport::new(candidates),
            Err(FzfError::DuplicateId(_))
        ));
    }

    #[test]
    fn transport_rejects_unsafe_fields() {
        let candidate = Candidate::new("bad\tid", "App")
            .with_action(CandidateAction::Exec(vec!["app".to_string()]));
        assert!(matches!(
            CandidateTransport::new(vec![candidate]),
            Err(FzfError::InvalidCandidateField("id"))
        ));
    }

    #[test]
    fn exact_matches_prefer_primary_name_over_generic_name() {
        let transport = CandidateTransport::new(vec![
            Candidate::new("desktop:1", "Alacritty").with_secondary_display_only("Terminal"),
            Candidate::new("desktop:2", "Terminal").with_secondary_display_only("Alacritty"),
        ])
        .unwrap();

        let matches = transport.exact_matches("alacritty", 10).unwrap();

        assert_eq!(matches[0].candidate.primary, "Alacritty");
        assert_eq!(matches[1].candidate.primary, "Terminal");
    }

    #[test]
    fn exact_matches_use_generic_name_when_primary_does_not_match() {
        let transport = CandidateTransport::new(vec![
            Candidate::new("desktop:1", "Alacritty").with_secondary_display_only("Terminal"),
            Candidate::new("desktop:2", "Firefox"),
        ])
        .unwrap();

        let matches = transport.exact_matches("terminal", 10).unwrap();

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].candidate.primary, "Alacritty");
    }
}
