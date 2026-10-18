use serde::{Deserialize, Serialize};

pub const CMD_SHOW: &str = "show";
pub const CMD_HIDE: &str = "hide";
pub const CMD_LIST: &str = "list";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub cmd: String,
    #[serde(rename = "id")]
    pub id: Option<String>,
    #[serde(rename = "icon")]
    pub icon: Option<String>,
    #[serde(rename = "tooltip")]
    pub tooltip: Option<String>,
    #[serde(rename = "on_click")]
    pub on_click: Option<String>,
    #[serde(rename = "pid")]
    pub pid: Option<i32>,
    #[serde(rename = "show_duration")]
    pub show_duration: Option<bool>,
    /// Replaces the item's context menu when set.
    #[serde(rename = "menu", default, skip_serializing_if = "Option::is_none")]
    pub menu: Option<Vec<MenuEntry>>,
}

/// One context menu row: a label and the shell command it runs. A label of
/// "-" with no command is a separator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MenuEntry {
    #[serde(rename = "label")]
    pub label: String,
    #[serde(rename = "command")]
    pub command: String,
}

impl MenuEntry {
    pub const SEPARATOR: &'static str = "-";

    pub fn is_separator(&self) -> bool {
        self.label == Self::SEPARATOR && self.command.is_empty()
    }

    /// Parses the CLI's `LABEL=COMMAND`, or `-` for a separator.
    pub fn parse(spec: &str) -> Result<Self, String> {
        if spec == Self::SEPARATOR {
            return Ok(Self { label: spec.to_string(), command: String::new() });
        }
        match spec.split_once('=') {
            Some((label, command)) if !label.trim().is_empty() && !command.trim().is_empty() => Ok(Self {
                label: label.trim().to_string(),
                command: command.trim().to_string(),
            }),
            _ => Err(format!("menu entry {:?} is not LABEL=COMMAND or -", spec)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    #[serde(rename = "ok")]
    pub ok: bool,
    #[serde(rename = "error")]
    pub error: Option<String>,
    #[serde(rename = "items")]
    pub items: Option<Vec<ItemInfo>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemInfo {
    #[serde(rename = "id")]
    pub id: String,
    #[serde(rename = "icon")]
    pub icon: String,
    #[serde(rename = "tooltip")]
    pub tooltip: String,
    #[serde(rename = "pid")]
    pub pid: Option<i32>,
}

impl Response {
    pub fn ok() -> Self {
        Self {
            ok: true,
            error: None,
            items: None,
        }
    }

    pub fn err(msg: impl Into<String>) -> Self {
        Self {
            ok: false,
            error: Some(msg.into()),
            items: None,
        }
    }

    pub fn with_items(items: Vec<ItemInfo>) -> Self {
        Self {
            ok: true,
            error: None,
            items: Some(items),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_menu_entries() {
        let e = MenuEntry::parse("Send files…=handoffd send --pick").unwrap();
        assert_eq!(e.label, "Send files…");
        assert_eq!(e.command, "handoffd send --pick");
        assert!(!e.is_separator());

        // Only the first = splits, so commands may hold their own
        let e = MenuEntry::parse("Env=FOO=1 run").unwrap();
        assert_eq!((e.label.as_str(), e.command.as_str()), ("Env", "FOO=1 run"));

        assert!(MenuEntry::parse("-").unwrap().is_separator());
        for bad in ["", "no command", "=cmd", "Label=", "  =  "] {
            assert!(MenuEntry::parse(bad).is_err(), "{:?} parsed", bad);
        }
    }

    #[test]
    fn requests_without_menu_still_parse() {
        // What a 0.2.1 CLI sends
        let req: Request = serde_json::from_str(
            r#"{"cmd":"show","id":"x","icon":null,"tooltip":"t","on_click":null,"pid":null,"show_duration":null}"#,
        )
        .unwrap();
        assert!(req.menu.is_none());
    }
}
