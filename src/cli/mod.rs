use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use std::io::{ErrorKind, Read, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::os::unix::process::ExitStatusExt;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crate::daemon::socket_path;
use crate::protocol::{Request, Response, CMD_HIDE, CMD_LIST, CMD_SHOW};
use signal_hook::consts::signal::{SIGINT, SIGTERM};
use signal_hook::flag;

enum ConnectMode {
    StartIfMissing,
    NoStart,
}

#[derive(Parser)]
#[command(name = "tray")]
#[command(version, disable_version_flag = true)]
#[command(about = "System tray helper for shell scripts", long_about = None)]
struct Cli {
    #[arg(
        short = 'v',
        long = "version",
        action = clap::ArgAction::Version,
        help = "Print version"
    )]
    _version: Option<bool>,
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
    Run {
        #[arg(long)]
        id: String,
        #[arg(long)]
        icon: Option<String>,
        #[arg(long)]
        tooltip: Option<String>,
        #[arg(long)]
        duration: bool,
        #[arg(long)]
        on_click: Option<String>,
        #[arg(
            required = true,
            num_args = 1..,
            trailing_var_arg = true,
            allow_hyphen_values = true
        )]
        command: Vec<String>,
    },
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
                show_duration: None,
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
                show_duration: None,
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
                show_duration: None,
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
        Commands::Run {
            id,
            icon,
            tooltip,
            duration,
            on_click,
            command,
        } => {
            if icon.is_none() && tooltip.is_none() && !duration {
                return Err(anyhow!("at least one of --icon or --tooltip is required"));
            }
            let code = run_with_tray(id, icon, tooltip, duration, on_click, command)?;
            std::process::exit(code);
        }
        Commands::Daemon => {
            crate::daemon::run()?;
        }
    }

    Ok(())
}

fn run_with_tray(
    id: String,
    icon: Option<String>,
    tooltip: Option<String>,
    duration: bool,
    on_click: Option<String>,
    command: Vec<String>,
) -> Result<i32> {
    let self_pid = std::process::id() as i32;
    send(
        Request {
            cmd: CMD_SHOW.to_string(),
            id: Some(id.clone()),
            icon,
            tooltip,
            on_click,
            // If tray exits unexpectedly, the watcher can still clean this item up.
            pid: Some(self_pid),
            show_duration: Some(duration),
        },
        ConnectMode::StartIfMissing,
    )?;

    let mut child = match Command::new(&command[0]).args(&command[1..]).spawn() {
        Ok(child) => child,
        Err(e) => {
            hide_best_effort(&id);
            return Err(e.into());
        }
    };

    let child_pid = child.id() as i32;
    let seen_signal = Arc::new(AtomicUsize::new(0));
    let sigint_id = flag::register_usize(SIGINT, seen_signal.clone(), SIGINT as usize)?;
    let sigterm_id = flag::register_usize(SIGTERM, seen_signal.clone(), SIGTERM as usize)?;

    let mut forwarded = false;
    let mut kill_deadline: Option<Instant> = None;

    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }

        let sig = seen_signal.load(Ordering::SeqCst) as i32;
        if sig != 0 && !forwarded {
            // Forward the same signal we received to the child process.
            let _ = unsafe { libc::kill(child_pid, sig) };
            forwarded = true;
            kill_deadline = Some(Instant::now() + Duration::from_secs(2));
        }

        if let Some(deadline) = kill_deadline {
            if Instant::now() >= deadline {
                let _ = unsafe { libc::kill(child_pid, libc::SIGKILL) };
                kill_deadline = None;
            }
        }

        thread::sleep(Duration::from_millis(50));
    };

    let _ = signal_hook::low_level::unregister(sigint_id);
    let _ = signal_hook::low_level::unregister(sigterm_id);
    hide_best_effort(&id);

    Ok(status_to_code(status))
}

fn hide_best_effort(id: &str) {
    let _ = send(
        Request {
            cmd: CMD_HIDE.to_string(),
            id: Some(id.to_string()),
            icon: None,
            tooltip: None,
            on_click: None,
            pid: None,
            show_duration: None,
        },
        ConnectMode::NoStart,
    );
}

fn status_to_code(status: std::process::ExitStatus) -> i32 {
    if let Some(code) = status.code() {
        code
    } else if let Some(sig) = status.signal() {
        128 + sig
    } else {
        1
    }
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
