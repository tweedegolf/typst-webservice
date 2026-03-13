use std::env;

use tokio::net::TcpListener;
use tracing::info;

use typst_webservice::{AppError, CRATE_INFO, logging, pdf::PdfContext, start_server};

const DEFAULT_ASSETS_DIR: &str = "assets";
const ASSETS_DIR_ENV_VAR: &str = "TWS_DIR";

const DEFAULT_PORT: u16 = 8080;
const DEFAULT_HOST: &str = "127.0.0.1";

const HOST_ENV_VAR: &str = "TWS_HOST";
const PORT_ENV_VAR: &str = "TWS_PORT";

#[cfg(test)]
mod cli_tests;

#[tokio::main]
/// Launch the HTTP server and publish the PDF rendering endpoint.
async fn main() -> Result<(), AppError> {
    let cli_args = parse_cli_args();
    if cli_args.show_version {
        println!("{CRATE_INFO}");
        return Ok(());
    }

    logging::init();
    info!("Starting Typst webservice");
    if !cli_args.extra.is_empty() {
        tracing::warn!(extra = ?cli_args.extra, "Ignoring unrecognized CLI arguments");
    }

    let assets_dir = resolve_assets_dir(cli_args.assets_dir);
    info!(%assets_dir, "Loading Typst assets");
    let pdf_context = PdfContext::from_directory(&assets_dir)?;

    let addr = resolve_addr(cli_args.addr);

    info!("Binding HTTP listener on {}", addr);
    let listener = TcpListener::bind(&addr).await?;

    start_server(listener, pdf_context).await
}

/// Determine the directory containing Typst assets from CLI args or environment.
fn resolve_assets_dir(assets_arg: Option<String>) -> String {
    assets_arg
        .filter(|arg| !arg.is_empty())
        .or_else(|| {
            env::var(ASSETS_DIR_ENV_VAR)
                .ok()
                .filter(|value| !value.is_empty())
        })
        .unwrap_or_else(|| DEFAULT_ASSETS_DIR.to_string())
}

fn resolve_addr(addr_arg: Option<AddrOverride>) -> String {
    let host = env::var(HOST_ENV_VAR).unwrap_or_else(|_| DEFAULT_HOST.to_string());
    let port = env::var(PORT_ENV_VAR)
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(DEFAULT_PORT);

    match addr_arg {
        Some(AddrOverride::Full(addr)) => addr,
        Some(AddrOverride::Port(port_override)) => format!("{host}:{port_override}"),
        None => format!("{host}:{port}"),
    }
}

#[derive(Debug)]
struct CliArgs {
    show_version: bool,
    assets_dir: Option<String>,
    addr: Option<AddrOverride>,
    extra: Vec<String>,
}

#[derive(Debug)]
enum AddrOverride {
    Full(String),
    Port(u16),
}

fn parse_cli_args() -> CliArgs {
    parse_cli_args_from(env::args().skip(1))
}

fn parse_cli_args_from<I, S>(args: I) -> CliArgs
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut show_version = false;
    let mut assets_dir = None;
    let mut addr = None;
    let mut extra = Vec::new();

    for arg in args {
        let arg = arg.into();
        if arg == "--version" || arg == "-v" {
            show_version = true;
            continue;
        }

        if addr.is_none()
            && let Some(parsed) = parse_addr_arg(&arg)
        {
            addr = Some(parsed);
            continue;
        }

        if assets_dir.is_none() {
            assets_dir = Some(arg);
            continue;
        }

        extra.push(arg);
    }

    CliArgs {
        show_version,
        assets_dir,
        addr,
        extra,
    }
}

fn parse_addr_arg(arg: &str) -> Option<AddrOverride> {
    if let Ok(port) = arg.parse::<u16>() {
        return Some(AddrOverride::Port(port));
    }

    let (host, port_str) = arg.rsplit_once(':')?;
    if host.is_empty() {
        return None;
    }

    let _ = port_str.parse::<u16>().ok()?;
    Some(AddrOverride::Full(arg.to_string()))
}
