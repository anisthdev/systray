use crate::protocol::{MenuEntry, Request};
use crate::sni::menu::{run_shell, DbusMenu};
use crate::sni::icon::resolve_icon;
use anyhow::Result;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use zbus::{interface, Connection};
use zbus::zvariant::OwnedObjectPath;

pub struct Item {
    pub id: String,
    pub icon: String,
    pub tooltip: String,
    pub description: String,
    pub on_click: String,
    pub pid: Option<i32>,
    pub show_duration: bool,
    pub started_at: Option<Instant>,
    pub menu: Vec<MenuEntry>,
    /// Bumped on every menu change, as dbusmenu's layout revision.
    pub menu_revision: u32,
}

impl Item {
    pub fn new(id: String, icon_str: &str, tooltip: &str, on_click: &str) -> Result<Self> {
        resolve_icon(icon_str)?;
        Ok(Self {
            id,
            icon: icon_str.to_string(),
            tooltip: tooltip.to_string(),
            description: String::new(),
            on_click: on_click.to_string(),
            pid: None,
            show_duration: false,
            started_at: None,
            menu: Vec::new(),
            menu_revision: 1,
        })
    }

    pub fn update(&mut self, req: &Request) -> Result<()> {
        if let Some(ref icon_str) = req.icon {
            resolve_icon(icon_str)?;
            self.icon = icon_str.clone();
        }
        if let Some(ref tooltip) = req.tooltip {
            self.tooltip = tooltip.clone();
        }
        if let Some(ref on_click) = req.on_click {
            self.on_click = on_click.clone();
        }
        if let Some(pid) = req.pid {
            self.pid = Some(pid);
        }
        if let Some(show_duration) = req.show_duration {
            self.show_duration = show_duration;
            if show_duration && self.started_at.is_none() {
                self.started_at = Some(Instant::now());
            }
        }
        if let Some(ref menu) = req.menu {
            if *menu != self.menu {
                self.menu = menu.clone();
                self.menu_revision += 1;
            }
        }
        Ok(())
    }

    pub fn set_duration_text(&mut self, duration: String) {
        self.description = duration;
    }
}

impl Clone for Item {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            icon: self.icon.clone(),
            tooltip: self.tooltip.clone(),
            description: self.description.clone(),
            on_click: self.on_click.clone(),
            pid: self.pid,
            show_duration: self.show_duration,
            started_at: self.started_at,
            menu: self.menu.clone(),
            menu_revision: self.menu_revision,
        }
    }
}

pub struct SniItem {
    conn: Connection,
    object_path: OwnedObjectPath,
    item: Arc<parking_lot::RwLock<Item>>,
}

impl SniItem {
    pub async fn new(
        id: String,
        object_path: String,
        icon_str: &str,
        tooltip: &str,
        on_click: &str,
        show_duration: bool,
        menu: Vec<MenuEntry>,
    ) -> Result<Self> {
        let conn = Connection::session().await?;
        let item = Arc::new(parking_lot::RwLock::new(Item::new(
            id, icon_str, tooltip, on_click,
        )?));
        if show_duration {
            item.write().show_duration = true;
            item.write().started_at = Some(Instant::now());
        }
        item.write().menu = menu;
        let menu_path: OwnedObjectPath = format!("{}/Menu", object_path).try_into()?;
        let object_path: OwnedObjectPath = object_path.try_into()?;
        conn.object_server()
            .at(
                object_path.as_str(),
                StatusNotifierItem { item: item.clone(), menu_path: menu_path.clone() },
            )
            .await?;
        // Served even while empty, so a menu given later needs no new path
        conn.object_server()
            .at(menu_path.as_str(), DbusMenu { item: item.clone() })
            .await?;

        let item = Self {
            conn,
            object_path,
            item,
        };

        if let Err(e) = item.register_with_watcher().await {
            log::warn!("tray registration failed for {}: {}", item.item.read().id, e);
        }

        Ok(item)
    }

    pub fn item(&self) -> Arc<parking_lot::RwLock<Item>> {
        self.item.clone()
    }

    pub async fn update(&self, req: &Request) -> Result<()> {
        let (had_menu, revision) = {
            let item = self.item.read();
            (!item.menu.is_empty(), item.menu_revision)
        };
        self.item.write().update(req)?;
        self.emit_tooltip_changed().await?;

        let (has_menu, new_revision) = {
            let item = self.item.read();
            (!item.menu.is_empty(), item.menu_revision)
        };
        if new_revision != revision {
            self.emit_layout_updated(new_revision).await?;
        }
        // Hosts read the Menu property once, at registration
        if has_menu != had_menu {
            self.register_with_watcher().await?;
        }
        Ok(())
    }

    async fn emit_layout_updated(&self, revision: u32) -> Result<()> {
        let path = format!("{}/Menu", self.object_path.as_str());
        let iface_ref = self.conn.object_server().interface::<_, DbusMenu>(path.as_str()).await?;
        DbusMenu::layout_updated(iface_ref.signal_context(), revision, 0).await?;
        Ok(())
    }

    pub async fn register_with_watcher(&self) -> Result<()> {
        self.conn
            .call_method(
                Some("org.kde.StatusNotifierWatcher"),
                "/StatusNotifierWatcher",
                Some("org.kde.StatusNotifierWatcher"),
                "RegisterStatusNotifierItem",
                &(self.object_path.as_str()),
            )
            .await?;
        Ok(())
    }

    pub async fn tick_duration(&self) -> Result<()> {
        let should_emit = {
            let mut item = self.item.write();
            if !item.show_duration {
                return Ok(());
            }

            let Some(started_at) = item.started_at else {
                item.started_at = Some(Instant::now());
                return Ok(());
            };

            item.set_duration_text(format_duration(Instant::now().duration_since(started_at)));
            true
        };

        if should_emit {
            self.emit_tooltip_changed().await?;
        }
        Ok(())
    }

    async fn emit_tooltip_changed(&self) -> Result<()> {
        let iface_ref = self
            .conn
            .object_server()
            .interface::<_, StatusNotifierItem>(self.object_path.as_str())
            .await?;
        StatusNotifierItem::new_tool_tip(iface_ref.signal_context()).await?;
        Ok(())
    }

    pub async fn remove(&self) {
        let _ = self
            .conn
            .object_server()
            .remove::<StatusNotifierItem, _>(self.object_path.clone())
            .await;
        let _ = self
            .conn
            .object_server()
            .remove::<DbusMenu, _>(format!("{}/Menu", self.object_path.as_str()).as_str())
            .await;
    }
}

struct StatusNotifierItem {
    item: Arc<parking_lot::RwLock<Item>>,
    menu_path: OwnedObjectPath,
}

#[interface(interface = "org.kde.StatusNotifierItem")]
impl StatusNotifierItem {
    #[zbus(signal)]
    async fn new_tool_tip(signal_ctxt: &zbus::object_server::SignalContext<'_>) -> zbus::Result<()>;

    fn activate(&self, _x: i32, _y: i32) {
        let on_click = self.item.read().on_click.clone();
        run_shell(&on_click);
    }

    fn context_menu(&self, _x: i32, _y: i32) {}

    fn secondary_activate(&self, _x: i32, _y: i32) {}

    fn scroll(&self, _delta: i32, _orientation: &str) {}

    #[zbus(property)]
    fn category(&self) -> &str {
        "ApplicationStatus"
    }

    #[zbus(property)]
    fn id(&self) -> String {
        self.item.read().id.clone()
    }

    #[zbus(property)]
    fn title(&self) -> String {
        let item = self.item.read();
        if item.tooltip.is_empty() {
            item.id.clone()
        } else {
            item.tooltip.clone()
        }
    }

    #[zbus(property)]
    fn tool_tip(&self) -> (String, Vec<(i32, i32, Vec<u8>)>, String, String) {
        let item = self.item.read();
        let title = if item.tooltip.is_empty() {
            item.id.clone()
        } else {
            item.tooltip.clone()
        };
        (
            String::new(),
            Vec::new(),
            title,
            item.description.clone(),
        )
    }

    #[zbus(property)]
    fn status(&self) -> &str {
        "Active"
    }

    #[zbus(property)]
    fn window_id(&self) -> u32 {
        0
    }

    #[zbus(property)]
    fn item_is_menu(&self) -> bool {
        false
    }

    #[zbus(property)]
    fn menu(&self) -> OwnedObjectPath {
        if self.item.read().menu.is_empty() {
            "/NO_DBUSMENU".try_into().expect("valid object path")
        } else {
            self.menu_path.clone()
        }
    }

    #[zbus(property)]
    fn icon_name(&self) -> String {
        let icon = self.item.read().icon.clone();
        let is_file_icon = !icon.is_empty() && (icon.contains('/') || Path::new(&icon).exists());
        if icon.is_empty() || is_file_icon {
            String::new()
        } else {
            icon
        }
    }

    #[zbus(property)]
    fn icon_pixmap(&self) -> Vec<(i32, i32, Vec<u8>)> {
        let icon = self.item.read().icon.clone();
        load_icon_pixmap(&icon)
    }
}

fn load_icon_pixmap(icon: &str) -> Vec<(i32, i32, Vec<u8>)> {
    if icon.is_empty() {
        return Vec::new();
    }
    if !(icon.contains('/') || Path::new(icon).exists()) {
        return Vec::new();
    }

    let Ok(img) = image::open(icon) else {
        return Vec::new();
    };
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();

    let mut argb = Vec::with_capacity((width * height * 4) as usize);
    for px in rgba.pixels() {
        argb.push(px[3]);
        argb.push(px[0]);
        argb.push(px[1]);
        argb.push(px[2]);
    }

    vec![(width as i32, height as i32, argb)]
}

fn format_duration(d: Duration) -> String {
    let total = d.as_secs();
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    format!("{h:02}:{m:02}:{s:02}")
}
