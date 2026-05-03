//! 持久化 pane 状态: state/panes.json + 每 session 的 .last_ai_byte

use crate::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaneInfo {
    pub selector: String,
    pub pane_id: u64,
    pub session_id: String,
    pub host: String,
    pub user: String,
    pub started_at: String,
    pub cast_path: PathBuf,
    pub sock_path: PathBuf,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct PanesState {
    /// project_root → { selector → PaneInfo }
    pub projects: HashMap<String, HashMap<String, PaneInfo>>,
}

pub fn load(state_dir: &std::path::Path) -> Result<PanesState> {
    let p = state_dir.join("panes.json");
    if !p.exists() {
        return Ok(PanesState::default());
    }
    let s = std::fs::read_to_string(&p)?;
    Ok(serde_json::from_str(&s).unwrap_or_default())
}

pub fn save(state_dir: &std::path::Path, st: &PanesState) -> Result<()> {
    std::fs::create_dir_all(state_dir)?;
    let p = state_dir.join("panes.json");
    let s = serde_json::to_string_pretty(st)?;
    std::fs::write(p, s)?;
    Ok(())
}