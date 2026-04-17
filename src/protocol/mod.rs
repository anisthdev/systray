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
