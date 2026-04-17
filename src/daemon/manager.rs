use crate::protocol::{ItemInfo, Request, Response, CMD_HIDE, CMD_LIST, CMD_SHOW};
use crate::sni::SniItem;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use zbus::Connection;

pub struct Manager {
    _conn: Connection,
    items: Mutex<HashMap<String, SniItem>>,
    counter: AtomicUsize,
    on_empty: Mutex<Option<Box<dyn Fn() + Send>>>,
}

impl Manager {
    pub async fn new(conn: Connection, on_empty: Box<dyn Fn() + Send>) -> Arc<Self> {
        Arc::new(Self {
            _conn: conn,
            items: Mutex::new(HashMap::new()),
            counter: AtomicUsize::new(0),
            on_empty: Mutex::new(Some(on_empty)),
        })
    }

    pub async fn show(&self, req: Request) -> Response {
        let id = match req.id.as_ref() {
            Some(id) => id,
            None => return Response::err("id is required"),
        };

        {
            let mut items = self.items.lock();
            if let Some(item) = items.get_mut(id) {
                if let Err(e) = item.update(&req) {
                    return Response::err(e.to_string());
                }
                return Response::ok();
            }
        }

        let counter = self.counter.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id() as i32;
        let bus_name = format!("org.kde.StatusNotifierItem-{}-{}", pid, counter);

        let sni_item = match SniItem::new(
            &self._conn,
            bus_name,
            id.clone(),
            req.icon.as_deref().unwrap_or(""),
            req.tooltip.as_deref().unwrap_or(""),
            req.on_click.as_deref().unwrap_or(""),
        )
        .await
        {
            Ok(item) => item,
            Err(e) => return Response::err(e.to_string()),
        };

        self.items.lock().insert(id.clone(), sni_item);

        log::info!("show item: {}", id);
        Response::ok()
    }

    pub async fn hide(&self, id: &str) -> Response {
        let removed = {
            let mut items = self.items.lock();
            items.remove(id)
        };

        match removed {
            Some(item) => {
                log::info!("hide item: {}", id);
                item.remove().await;

                let items = self.items.lock();
                if items.is_empty() {
                    if let Some(ref on_empty) = *self.on_empty.lock() {
                        on_empty();
                    }
                }
                Response::ok()
            }
            None => Response::err(format!("no item with id {:?}", id)),
        }
    }

    pub async fn hide_pid(&self, pid: i32) {
        let (removed_items, empty) = {
            let mut items = self.items.lock();
            let to_remove: Vec<String> = items
                .iter()
                .filter(|(_, item)| item.item().read().pid == Some(pid))
                .map(|(id, _)| id.clone())
                .collect();
            let mut removed_items = Vec::new();

            for id in to_remove {
                if let Some(item) = items.remove(&id) {
                    log::info!("hide item (pid {}): {}", pid, id);
                    removed_items.push(item);
                }
            }
            (removed_items, items.is_empty())
        };

        for item in removed_items {
            item.remove().await;
        }

        if empty {
            if let Some(ref on_empty) = *self.on_empty.lock() {
                on_empty();
            }
        }
    }

    pub fn list(&self) -> Vec<ItemInfo> {
        let items = self.items.lock();
        items
            .values()
            .map(|item| {
                let item_arc = item.item();
                let si = item_arc.read();
                ItemInfo {
                    id: si.id.clone(),
                    icon: si.icon.clone(),
                    tooltip: si.tooltip.clone(),
                    pid: si.pid,
                }
            })
            .collect()
    }

    pub fn watched_pids(&self) -> Vec<i32> {
        let items = self.items.lock();
        let mut pids = Vec::new();
        for item in items.values() {
            if let Some(pid) = item.item().read().pid {
                pids.push(pid);
            }
        }
        pids
    }

    pub async fn dispatch(&self, req: Request) -> Response {
        match req.cmd.as_str() {
            CMD_SHOW => self.show(req).await,
            CMD_HIDE => {
                if let Some(ref id) = req.id {
                    self.hide(id).await
                } else {
                    Response::err("id is required for hide".to_string())
                }
            }
            CMD_LIST => Response::with_items(self.list()),
            _ => Response::err(format!("unknown command {:?}", req.cmd)),
        }
    }
}
