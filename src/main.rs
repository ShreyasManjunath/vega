use std::env;
use std::io::{self, IsTerminal, Read};
use std::time::Duration;

use vega::fzf::{FzfBackend, FzfConfig, FzfError, QueryRequest, resolve_binary_path};
use vega::gui::{LauncherOptions, run_launcher};
use vega::modes::{DesktopMode, DmenuMode, Mode, RunMode};
use vega::settings::SettingsManager;

fn main() {
    if let Err(error) = run() {
        eprintln!("vega: {error}");
        std::process::exit(error.exit_code());
    }
}

fn run() -> Result<(), AppError> {
    let raw_args: Vec<String> = env::args().skip(1).collect();
    if raw_args
        .iter()
        .any(|arg| matches!(arg.as_str(), "-h" | "--help"))
    {
        print_help();
        return Ok(());
    }

    let settings_manager = SettingsManager::load()?;
    let settings = settings_manager.current();
    let args = Args::parse(raw_args.into_iter())?;
    if args.help {
        print_help();
        return Ok(());
    }

    let config = &settings.config;
    let mode_name = args
        .mode
        .clone()
        .unwrap_or_else(|| config.behavior.default_mode.clone());
    let mode: Box<dyn Mode> = match mode_name.as_str() {
        "dmenu" => Box::new(DmenuMode::new(read_stdin()?)),
        "cmd" | "run" => Box::new(RunMode::new()),
        "apps" | "drun" => Box::new(DesktopMode::new()),
        other => return Err(AppError::Usage(format!("unsupported mode `{other}`"))),
    };

    let limit = args.limit.unwrap_or(config.runtime.limit);
    let debug = args.debug || config.runtime.debug;
    let timeout = args
        .timeout
        .unwrap_or_else(|| Duration::from_millis(config.runtime.timeout_ms));
    let fzf_binary = args
        .fzf_binary
        .unwrap_or_else(|| config.runtime.fzf_binary.clone());
    let mut fzf_flags = config.runtime.fzf_flags.clone();
    fzf_flags.extend(args.fzf_flags);

    let fzf_config = FzfConfig {
        binary: fzf_binary,
        timeout,
        extra_flags: fzf_flags,
    };

    if debug {
        let resolved = resolve_binary_path(&fzf_config.binary)
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "<not found on PATH>".to_string());
        eprintln!(
            "vega: fzf binary={} resolved={}",
            fzf_config.binary, resolved
        );
    }

    if args.query.is_none() {
        return run_launcher(LauncherOptions {
            mode_name: mode.name().to_string(),
            mode,
            fzf_config,
            debug,
            settings_manager,
        })
        .map_err(AppError::Gui);
    }

    let candidates = mode.load()?;
    let backend = FzfBackend::start(fzf_config)?;
    let query = args.query.unwrap_or_default();
    let response = backend.query(QueryRequest {
        query,
        candidates,
        limit,
    })?;

    if debug {
        eprintln!(
            "vega: mode={} candidates={} results={} elapsed_ms={}",
            mode.name(),
            response.candidate_count,
            response.matches.len(),
            response.elapsed.as_millis()
        );
    }

    let Some(selected) = response.matches.first() else {
        return Ok(());
    };

    if args.execute {
        mode.execute(&selected.candidate)?;
    } else {
        println!("{}", selected.candidate.primary);
    }

    Ok(())
}

fn read_stdin() -> Result<String, AppError> {
    if io::stdin().is_terminal() {
        return Ok(String::new());
    }

    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    Ok(input)
}

fn print_help() {
    println!(
        "vega - launcher backed by managed fzf\n\
\n\
Usage:\n\
  vega -show <dmenu|cmd|apps> [--query TEXT] [--limit N]\n\
\n\
Examples:\n\
  vega -show cmd\n\
  printf 'Firefox\\nFiles\\nTerminal\\n' | vega -show dmenu --query fire\n\
  vega -show cmd --query alacritty --execute\n\
  vega -show apps --query browser\n\
\n\
Options:\n\
  -show MODE       Select mode: dmenu, cmd, or apps\n\
  --query TEXT     Run non-interactively with fzf --filter\n\
  --limit N        Maximum parsed results, default 20\n\
  --execute        Execute the first non-interactive match without a shell\n\
  --fzf PATH       fzf binary path, default fzf\n\
  --fzf-flag FLAG  Extra flag passed directly to fzf\n\
  --timeout MS     Query timeout, default 1500\n\
  --debug          Print backend diagnostics to stderr\n\
  -h, --help       Show this help"
    );
}

#[derive(Debug)]
struct Args {
    mode: Option<String>,
    query: Option<String>,
    limit: Option<usize>,
    execute: bool,
    debug: bool,
    help: bool,
    timeout: Option<Duration>,
    fzf_binary: Option<String>,
    fzf_flags: Vec<String>,
}

impl Args {
    fn parse<I>(mut input: I) -> Result<Self, AppError>
    where
        I: Iterator<Item = String>,
    {
        let mut args = Self {
            mode: None,
            query: None,
            limit: None,
            execute: false,
            debug: false,
            help: false,
            timeout: None,
            fzf_binary: None,
            fzf_flags: Vec::new(),
        };

        while let Some(arg) = input.next() {
            match arg.as_str() {
                "-h" | "--help" => args.help = true,
                "-show" | "--show" => args.mode = Some(next_value(&mut input, &arg)?),
                "--query" | "-q" => args.query = Some(next_value(&mut input, &arg)?),
                "--limit" => {
                    args.limit = Some(next_value(&mut input, &arg)?.parse().map_err(|_| {
                        AppError::Usage("--limit must be a positive integer".into())
                    })?);
                }
                "--execute" => args.execute = true,
                "--debug" => args.debug = true,
                "--fzf" => args.fzf_binary = Some(next_value(&mut input, &arg)?),
                "--fzf-flag" => args.fzf_flags.push(next_value(&mut input, &arg)?),
                "--timeout" => {
                    let ms: u64 = next_value(&mut input, &arg)?
                        .parse()
                        .map_err(|_| AppError::Usage("--timeout must be milliseconds".into()))?;
                    args.timeout = Some(Duration::from_millis(ms));
                }
                unknown => return Err(AppError::Usage(format!("unknown argument `{unknown}`"))),
            }
        }

        if args.limit == Some(0) {
            return Err(AppError::Usage("--limit must be greater than zero".into()));
        }

        Ok(args)
    }
}

fn next_value<I>(input: &mut I, flag: &str) -> Result<String, AppError>
where
    I: Iterator<Item = String>,
{
    input
        .next()
        .ok_or_else(|| AppError::Usage(format!("{flag} requires a value")))
}

#[derive(Debug)]
enum AppError {
    Usage(String),
    Io(io::Error),
    Config(vega::settings::SettingsError),
    Fzf(FzfError),
    Mode(vega::modes::ModeError),
    Gui(String),
}

impl AppError {
    fn exit_code(&self) -> i32 {
        match self {
            Self::Usage(_) => 2,
            Self::Io(_) | Self::Config(_) | Self::Fzf(_) | Self::Mode(_) | Self::Gui(_) => 1,
        }
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Usage(message) => write!(formatter, "{message}"),
            Self::Io(error) => write!(formatter, "{error}"),
            Self::Config(error) => write!(formatter, "{error}"),
            Self::Fzf(error) => write!(formatter, "{error}"),
            Self::Mode(error) => write!(formatter, "{error}"),
            Self::Gui(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for AppError {}

impl From<io::Error> for AppError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<FzfError> for AppError {
    fn from(error: FzfError) -> Self {
        Self::Fzf(error)
    }
}

impl From<vega::settings::SettingsError> for AppError {
    fn from(error: vega::settings::SettingsError) -> Self {
        Self::Config(error)
    }
}

impl From<vega::modes::ModeError> for AppError {
    fn from(error: vega::modes::ModeError) -> Self {
        Self::Mode(error)
    }
}
