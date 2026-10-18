//! The item's context menu over `com.canonical.dbusmenu`, which tray hosts
//! (waybar, KDE, GNOME's AppIndicator extension) open on a right click.
//! The menu is flat: the root (id 0) holds one row per entry, ids 1..=n.

use crate::protocol::MenuEntry;
use crate::sni::item::Item;
use std::collections::HashMap;
use std::sync::Arc;
use zbus::fdo;
use zbus::interface;
use zbus::zvariant::{OwnedValue, StructureBuilder, Type, Value};

pub struct DbusMenu {
    pub item: Arc<parking_lot::RwLock<Item>>,
}

/// Runs a shell command without waiting for it; the child is reaped on a
/// thread so finished commands don't linger as zombies.
pub fn run_shell(command: &str) {
    if command.is_empty() {
        return;
    }
    match std::process::Command::new("sh").arg("-lc").arg(command).spawn() {
        Ok(mut child) => {
            std::thread::spawn(move || child.wait());
        }
        Err(e) => log::warn!("failed to run {:?}: {}", command, e),
    }
}

fn props(entry: &MenuEntry) -> HashMap<String, Value<'static>> {
    let mut p = HashMap::new();
    if entry.is_separator() {
        p.insert("type".to_string(), Value::from("separator"));
    } else {
        p.insert("label".to_string(), Value::from(entry.label.clone()));
        p.insert("enabled".to_string(), Value::from(true));
        p.insert("visible".to_string(), Value::from(true));
    }
    p
}

fn root_props() -> HashMap<String, Value<'static>> {
    HashMap::from([("children-display".to_string(), Value::from("submenu"))])
}

/// Only the properties asked for; none asked means all of them.
fn filtered(p: HashMap<String, Value<'static>>, names: &[String]) -> HashMap<String, OwnedValue> {
    p.into_iter()
        .filter(|(k, _)| names.is_empty() || names.contains(k))
        .filter_map(|(k, v)| OwnedValue::try_from(v).ok().map(|v| (k, v)))
        .collect()
}

/// The layout GetLayout returns, `(ia{sv}av)`: an item, its properties, and
/// its children, each a variant holding the same structure.
#[derive(serde::Serialize, Type)]
pub struct Layout(i32, HashMap<String, OwnedValue>, Vec<OwnedValue>);

/// A child of the layout: an item with no children of its own.
fn child(id: i32, p: HashMap<String, OwnedValue>) -> Option<OwnedValue> {
    let v: Value = StructureBuilder::new()
        .add_field(id)
        .append_field(Value::from(p))
        .append_field(Value::from(Vec::<Value>::new()))
        .build()
        .into();
    OwnedValue::try_from(v).ok()
}

impl DbusMenu {
    fn entry(&self, id: i32) -> Option<MenuEntry> {
        let index = usize::try_from(id).ok()?.checked_sub(1)?;
        self.item.read().menu.get(index).cloned()
    }

    fn props_of(&self, id: i32) -> Option<HashMap<String, Value<'static>>> {
        if id == 0 {
            Some(root_props())
        } else {
            self.entry(id).map(|e| props(&e))
        }
    }

    fn clicked(&self, id: i32, event: &str) {
        if event != "clicked" {
            return;
        }
        if let Some(entry) = self.entry(id) {
            run_shell(&entry.command);
        }
    }
}

#[interface(name = "com.canonical.dbusmenu")]
impl DbusMenu {
    #[zbus(out_args("revision", "layout"))]
    fn get_layout(
        &self,
        parent_id: i32,
        recursion_depth: i32,
        property_names: Vec<String>,
    ) -> fdo::Result<(u32, Layout)> {
        let (revision, menu) = {
            let item = self.item.read();
            (item.menu_revision, item.menu.clone())
        };
        if parent_id == 0 {
            let children = if recursion_depth == 0 {
                Vec::new()
            } else {
                menu.iter()
                    .enumerate()
                    .filter_map(|(i, e)| child(i as i32 + 1, filtered(props(e), &property_names)))
                    .collect()
            };
            return Ok((revision, Layout(0, filtered(root_props(), &property_names), children)));
        }
        match self.entry(parent_id) {
            Some(e) => Ok((revision, Layout(parent_id, filtered(props(&e), &property_names), Vec::new()))),
            None => Err(fdo::Error::InvalidArgs(format!("no menu item {}", parent_id))),
        }
    }

    fn get_group_properties(
        &self,
        ids: Vec<i32>,
        property_names: Vec<String>,
    ) -> Vec<(i32, HashMap<String, OwnedValue>)> {
        let ids = if ids.is_empty() {
            (0..=self.item.read().menu.len() as i32).collect()
        } else {
            ids
        };
        ids.into_iter()
            .filter_map(|id| self.props_of(id).map(|p| (id, filtered(p, &property_names))))
            .collect()
    }

    fn get_property(&self, id: i32, name: String) -> fdo::Result<OwnedValue> {
        self.props_of(id)
            .and_then(|mut p| p.remove(&name))
            .and_then(|v| OwnedValue::try_from(v).ok())
            .ok_or_else(|| fdo::Error::InvalidArgs(format!("no property {:?} on menu item {}", name, id)))
    }

    fn event(&self, id: i32, event_id: String, _data: OwnedValue, _timestamp: u32) {
        self.clicked(id, &event_id);
    }

    fn event_group(&self, events: Vec<(i32, String, OwnedValue, u32)>) -> Vec<i32> {
        let mut errors = Vec::new();
        for (id, event_id, _, _) in events {
            if self.props_of(id).is_none() {
                errors.push(id);
            } else {
                self.clicked(id, &event_id);
            }
        }
        errors
    }

    fn about_to_show(&self, _id: i32) -> bool {
        false
    }

    #[zbus(out_args("updates_needed", "id_errors"))]
    fn about_to_show_group(&self, _ids: Vec<i32>) -> (Vec<i32>, Vec<i32>) {
        (Vec::new(), Vec::new())
    }

    #[zbus(signal)]
    pub async fn layout_updated(
        signal_ctxt: &zbus::object_server::SignalContext<'_>,
        revision: u32,
        parent: i32,
    ) -> zbus::Result<()>;

    #[zbus(property)]
    fn version(&self) -> u32 {
        3
    }

    #[zbus(property)]
    fn text_direction(&self) -> &str {
        "ltr"
    }

    #[zbus(property)]
    fn status(&self) -> &str {
        "normal"
    }

    #[zbus(property)]
    fn icon_theme_path(&self) -> Vec<String> {
        Vec::new()
    }
}
