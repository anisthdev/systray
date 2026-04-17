use crate::protocol::Request;
use crate::sni::icon::resolve_icon;
use anyhow::Result;
use std::path::Path;
use std::sync::Arc;
use zbus::{interface, Connection};
use zbus::zvariant::OwnedObjectPath;

pub struct Item {
    pub id: String,
    pub icon: String,
    pub tooltip: String,
    pub on_click: String,
    pub pid: Option<i32>,
}

impl Item {
    pub fn new(id: String, icon_str: &str, tooltip: &str, on_click: &str) -> Result<Self> {
        resolve_icon(icon_str)?;
        Ok(Self {
            id,
            icon: icon_str.to_string(),
            tooltip: tooltip.to_string(),
            on_click: on_click.to_string(),
            pid: None,
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
        Ok(())
    }
}

impl Clone for Item {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            icon: self.icon.clone(),
            tooltip: self.tooltip.clone(),
            on_click: self.on_click.clone(),
            pid: self.pid,
        }
    }
}

pub struct SniItem {
    conn: Connection,
    bus_name: String,
    object_path: OwnedObjectPath,
    item: Arc<parking_lot::RwLock<Item>>,
}

impl SniItem {
    pub async fn new(
        conn: &Connection,
        bus_name: String,
        id: String,
        icon_str: &str,
        tooltip: &str,
        on_click: &str,
    ) -> Result<Self> {
        let item = Arc::new(parking_lot::RwLock::new(Item::new(
            id, icon_str, tooltip, on_click,
        )?));
        let object_path: OwnedObjectPath = "/StatusNotifierItem".try_into()?;

        conn.request_name(bus_name.as_str()).await?;
        conn.object_server()
            .at(object_path.as_str(), StatusNotifierItem { item: item.clone() })
            .await?;

        let _ = conn
            .call_method(
                Some("org.kde.StatusNotifierWatcher"),
                "/StatusNotifierWatcher",
                Some("org.kde.StatusNotifierWatcher"),
                "RegisterStatusNotifierItem",
                &(bus_name.as_str()),
            )
            .await;

        Ok(Self {
            conn: conn.clone(),
            bus_name,
            object_path,
            item,
        })
    }

    pub fn item(&self) -> Arc<parking_lot::RwLock<Item>> {
        self.item.clone()
    }

    pub fn update(&self, req: &Request) -> Result<()> {
        self.item.write().update(req)
    }

    pub async fn remove(self) {
        let _ = self
            .conn
            .object_server()
            .remove::<StatusNotifierItem, _>(self.object_path.clone())
            .await;
        let _ = self.conn.release_name(self.bus_name.as_str()).await;
    }
}

struct StatusNotifierItem {
    item: Arc<parking_lot::RwLock<Item>>,
}

#[interface(interface = "org.kde.StatusNotifierItem")]
impl StatusNotifierItem {
    fn activate(&self, _x: i32, _y: i32) {
        let on_click = self.item.read().on_click.clone();
        if on_click.is_empty() {
            return;
        }
        let _ = std::process::Command::new("sh")
            .arg("-lc")
            .arg(on_click)
            .spawn();
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
        "/NO_DBUSMENU".try_into().expect("valid object path")
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
