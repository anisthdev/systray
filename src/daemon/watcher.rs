use crate::daemon::Manager;
use anyhow::Result;
use zbus::fdo::DBusProxy;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::Instant as TokioInstant;

const PID_INTERVAL: Duration = Duration::from_secs(2);
const DURATION_INTERVAL: Duration = Duration::from_secs(1);
const TRAY_REPLAY_INTERVAL: Duration = Duration::from_secs(3);
const TRAY_WATCHER_NAME: &str = "org.kde.StatusNotifierWatcher";

pub async fn start_watcher_async(
    mgr: Arc<Manager>,
    done: Arc<std::sync::atomic::AtomicBool>,
) {
    let mgr3 = mgr.clone();
    let done3 = done.clone();
    tokio::spawn(async move {
        let mut pid_tick = tokio::time::interval_at(TokioInstant::now() + PID_INTERVAL, PID_INTERVAL);
        let mut duration_tick = tokio::time::interval_at(TokioInstant::now() + DURATION_INTERVAL, DURATION_INTERVAL);
        let mut tray_tick = tokio::time::interval_at(TokioInstant::now() + TRAY_REPLAY_INTERVAL, TRAY_REPLAY_INTERVAL);
        let conn = match zbus::Connection::session().await {
            Ok(conn) => conn,
            Err(e) => {
                log::warn!("tray host watcher disabled: failed to connect to session bus: {}", e);
                return;
            }
        };
        let dbus = match DBusProxy::new(&conn).await {
            Ok(proxy) => proxy,
            Err(e) => {
                log::warn!("tray host watcher disabled: failed to create DBus proxy: {}", e);
                return;
            }
        };

        loop {
            tokio::select! {
                _ = pid_tick.tick() => {
                    if done3.load(std::sync::atomic::Ordering::Relaxed) {
                        return;
                    }
                    let pids = mgr3.watched_pids();
                    for pid in pids {
                        if !pid_alive(pid) {
                            mgr3.hide_pid(pid).await;
                        }
                    }
                }
                _ = duration_tick.tick() => {
                    if done3.load(std::sync::atomic::Ordering::Relaxed) {
                        return;
                    }
                    mgr3.tick_durations().await;
                }
                _ = tray_tick.tick() => {
                    if done3.load(std::sync::atomic::Ordering::Relaxed) {
                        return;
                    }

                    match tray_host_has_owner(&dbus).await {
                        Ok(true) => {
                            if mgr3.has_items() {
                                mgr3.re_register_all().await;
                            }
                        }
                        Ok(false) => {}
                        Err(e) => {
                            log::warn!("tray host watcher check failed: {}", e);
                        }
                    }
                }
            }
        }
    });
}

async fn tray_host_has_owner(dbus: &DBusProxy<'_>) -> Result<bool> {
    Ok(dbus.name_has_owner(TRAY_WATCHER_NAME.try_into()?).await?)
}

fn pid_alive(pid: i32) -> bool {
    let path = format!("/proc/{}", pid);
    std::fs::metadata(&path).is_ok()
}
