use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use std::io::{ErrorKind, Read, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use crate::daemon::socket_path;
use crate::protocol::{Request, Response, CMD_HIDE, CMD_LIST, CMD_SHOW};

enum ConnectMode {
    StartIfMissing,
    NoStart,
}

#[derive(Parser)]
#[command(name = "tray")]
#[command(about = "System tray helper for shell scripts", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Show {
        #[arg(long)]
        id: String,
        #[arg(long)]
        icon: Option<String>,
        #[arg(long)]
        tooltip: Option<String>,
        #[arg(long)]
        on_click: Option<String>,
        #[arg(long)]
        pid: Option<i32>,
    },
    Hide {
        #[arg(long)]
        id: String,
    },
    List,
    Daemon,
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Show {
            id,
            icon,
            tooltip,
            on_click,
            pid,
        } => {
            if icon.is_none() && tooltip.is_none() {
                return Err(anyhow!("at least one of --icon or --tooltip is required"));
            }

            send(Request {
                cmd: CMD_SHOW.to_string(),
                id: Some(id),
                icon,
                tooltip,
                on_click,
                pid,
            }, ConnectMode::StartIfMissing)?;
        }
        Commands::Hide { id } => {
            let resp = send(Request {
                cmd: CMD_HIDE.to_string(),
                id: Some(id),
                icon: None,
                tooltip: None,
                on_click: None,
                pid: None,
            }, ConnectMode::NoStart)?;

            if !resp.ok {
                return Err(anyhow!("{}", resp.error.unwrap_or_else(|| "hide failed".to_string())));
            }
        }
        Commands::List => {
            let resp = send(Request {
                cmd: CMD_LIST.to_string(),
                id: None,
                icon: None,
                tooltip: None,
                on_click: None,
                pid: None,
            }, ConnectMode::NoStart)?;

            if resp.ok {
                if let Some(items) = resp.items {
                    for item in items {
                        println!("{}", item.id);
                    }
                }
            } else {
                return Err(anyhow!("{:?}", resp.error));
            }
        }
        Commands::Daemon => {
            crate::daemon::run()?;
        }
    }

    Ok(())
}

fn send(req: Request, mode: ConnectMode) -> Result<Response> {
    let path = socket_path();

    let mut stream = match UnixStream::connect(&path) {
        Ok(stream) => stream,
        Err(e) if matches!(e.kind(), ErrorKind::NotFound | ErrorKind::ConnectionRefused) => {
            remove_stale_socket(&path);
            match mode {
                ConnectMode::StartIfMissing => {
                    spawn_daemon()?;
                    wait_for_daemon(&path)?
                }
                ConnectMode::NoStart => return Ok(default_response(&req)),
            }
        }
        Err(e) => return Err(e.into()),
    };
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;

    let req_bytes = serde_json::to_vec(&req)?;
    stream.write_all(&req_bytes)?;
    stream.flush()?;
    stream.shutdown(Shutdown::Write)?;

    let mut resp_bytes = Vec::new();
    stream.read_to_end(&mut resp_bytes)?;

    let resp: Response = serde_json::from_slice(&resp_bytes)?;
    Ok(resp)
}

fn default_response(req: &Request) -> Response {
    match req.cmd.as_str() {
        CMD_LIST => Response::with_items(Vec::new()),
        CMD_HIDE => Response::err(match req.id.as_deref() {
            Some(id) => format!("no item with id {:?}", id),
            None => "id is required for hide".to_string(),
        }),
        _ => Response::err("daemon is not running"),
    }
}

fn spawn_daemon() -> Result<()> {
    let exe = std::env::current_exe()?;
    let mut child = Command::new(exe);
    child.arg("daemon");
    child.stdin(Stdio::null());
    child.stdout(Stdio::null());
    child.stderr(Stdio::null());
    child.spawn()?;
    Ok(())
}

fn remove_stale_socket(path: &str) {
    let _ = std::fs::remove_file(path);
}

fn wait_for_daemon(path: &str) -> Result<UnixStream> {
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    let mut last_err: Option<std::io::Error> = None;

    while std::time::Instant::now() < deadline {
        match UnixStream::connect(path) {
            Ok(stream) => return Ok(stream),
            Err(e) => {
                last_err = Some(e);
                thread::sleep(Duration::from_millis(50));
            }
        }
    }

    match last_err {
        Some(e) => Err(anyhow!("daemon did not start within 3s: {}", e)),
        None => Err(anyhow!("daemon did not start within 3s")),
    }
}
