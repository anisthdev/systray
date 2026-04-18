use crate::daemon::Manager;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::Instant as TokioInstant;

const PID_INTERVAL: Duration = Duration::from_secs(2);
const DURATION_INTERVAL: Duration = Duration::from_secs(1);

pub async fn start_watcher_async(
    mgr: Arc<Manager>,
    done: Arc<std::sync::atomic::AtomicBool>,
) {
    let mgr3 = mgr.clone();
    let done3 = done.clone();
    tokio::spawn(async move {
        let mut pid_tick = tokio::time::interval_at(TokioInstant::now() + PID_INTERVAL, PID_INTERVAL);
        let mut duration_tick = tokio::time::interval_at(TokioInstant::now() + DURATION_INTERVAL, DURATION_INTERVAL);
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
            }
        }
    });
}

fn pid_alive(pid: i32) -> bool {
    let path = format!("/proc/{}", pid);
    std::fs::metadata(&path).is_ok()
}
