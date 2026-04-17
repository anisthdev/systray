use crate::daemon::Manager;
use std::sync::Arc;
use std::time::Duration;

const PID_INTERVAL: Duration = Duration::from_secs(2);

pub async fn start_watcher_async(
    mgr: Arc<Manager>,
    done: Arc<std::sync::atomic::AtomicBool>,
) {
    let mgr3 = mgr.clone();
    let done3 = done.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(PID_INTERVAL).await;
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
    });
}

fn pid_alive(pid: i32) -> bool {
    let path = format!("/proc/{}", pid);
    std::fs::metadata(&path).is_ok()
}
