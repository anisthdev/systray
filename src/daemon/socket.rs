use crate::daemon::Manager;
use crate::protocol::Request;
use anyhow::Result;
use std::sync::atomic::{AtomicBool, Ordering};
use std::path::Path;
use std::sync::Arc;
use std::{fs, io};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};

pub fn socket_path() -> String {
    if let Some(dir) = std::env::var_os("XDG_RUNTIME_DIR") {
        return format!("{}/tray.sock", dir.to_string_lossy());
    }
    format!("/tmp/tray-{}/tray.sock", unsafe { libc::geteuid() })
}

pub fn remove_stale_socket(path: &str) {
    if let Some(parent) = Path::new(path).parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::remove_file(path);
}

pub async fn run_socket_server(
    mgr: Arc<Manager>,
    stop: Arc<AtomicBool>,
    shutdown: Arc<tokio::sync::Notify>,
) -> Result<()> {
    let path = socket_path();
    remove_stale_socket(&path);

    let listener = UnixListener::bind(&path)?;
    log::info!("socket listening on {}", path);

    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }

        tokio::select! {
            _ = shutdown.notified() => {
                break;
            }
            accept = listener.accept() => {
                match accept {
                    Ok((stream, _)) => {
                        let mgr = mgr.clone();
                        tokio::spawn(async move {
                            if let Err(e) = handle_conn(stream, &mgr).await {
                                log::error!("connection error: {}", e);
                            }
                        });
                    }
                    Err(e) => {
                        if e.kind() != io::ErrorKind::Interrupted {
                            log::error!("accept error: {}", e);
                        }
                    }
                }
            }
        }
    }

    let _ = fs::remove_file(&path);
    Ok(())
}

async fn handle_conn(mut stream: UnixStream, mgr: &Manager) -> Result<()> {
    log::debug!("handle_conn: new connection");

    let mut buf = Vec::new();
    match stream.read_to_end(&mut buf).await {
        Ok(n) => log::debug!("handle_conn: read {} bytes", n),
        Err(e) => {
            log::error!("handle_conn: read error: {}", e);
            return Err(e.into());
        }
    }

    if buf.is_empty() {
        log::error!("handle_conn: empty buffer, peer may have closed prematurely");
        return Err(anyhow::anyhow!("empty request").into());
    }

    let req: Request = serde_json::from_slice(&buf)?;
    log::debug!("handle_conn: parsed request: {:?}", req.cmd);

    let resp = mgr.dispatch(req).await;
    let resp_bytes = serde_json::to_vec(&resp)?;

    stream.write_all(&resp_bytes).await?;
    stream.flush().await?;
    log::debug!("handle_conn: wrote response");

    Ok(())
}
