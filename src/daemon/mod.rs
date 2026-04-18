pub mod manager;
pub mod socket;
pub mod watcher;

pub use manager::Manager;
pub use socket::{run_socket_server, socket_path};
pub use watcher::start_watcher_async;

use anyhow::Result;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Notify;

pub fn run() -> Result<()> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async_run())
}

async fn async_run() -> Result<()> {
    let stop = Arc::new(AtomicBool::new(false));
    let shutdown = Arc::new(Notify::new());
    let stop_clone = stop.clone();
    let shutdown_clone = shutdown.clone();

    let on_empty = Box::new(move || {
        log::info!("all items removed, shutting down");
        stop_clone.store(true, Ordering::Relaxed);
        shutdown_clone.notify_one();
    });

    let mgr = Manager::new(on_empty).await;

    let mgr_clone = mgr.clone();
    let stop_clone2 = stop.clone();
    start_watcher_async(mgr_clone, stop_clone2).await;

    let stop_clone3 = stop.clone();
    let shutdown_clone2 = shutdown.clone();

    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        log::info!("received shutdown signal");
        stop_clone3.store(true, Ordering::Relaxed);
        shutdown_clone2.notify_one();
    });

    let mgr2 = mgr.clone();
    run_socket_server(mgr2, stop, shutdown).await
}
