//! 持久化 pane 状态: SSHOPS_HOME/state/panes.json
//!
//! schema (跟 bash 版兼容, 不要改 key):
//! ```json
//! {
//!   "<project_id>": {
//!     "wezterm_window_id": 17,
//!     "started_at": "2026-05-02T10:00:00Z",
//!     "panes": {
//!       "<selector>": { "pane_id": 42, "session_id": "...", "started_at": "..." }
//!     }
//!   }
//! }
//! ```

use crate::Result;
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaneInfo {
    pub pane_id: u64,
    pub session_id: String,
    pub started_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectState {
    #[serde(default)]
    pub wezterm_window_id: u64,
    #[serde(default)]
    pub started_at: String,
    #[serde(default)]
    pub panes: HashMap<String, PaneInfo>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct PanesState {
    #[serde(flatten)]
    pub projects: HashMap<String, ProjectState>,
}

pub struct StateStore {
    path: PathBuf,
}

impl StateStore {
    pub fn new(state_dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(state_dir)?;
        Ok(Self {
            path: state_dir.join("panes.json"),
        })
    }

    pub fn read(&self) -> Result<PanesState> {
        if !self.path.exists() {
            return Ok(PanesState::default());
        }
        let s = std::fs::read_to_string(&self.path)?;
        if s.trim().is_empty() {
            return Ok(PanesState::default());
        }
        Ok(serde_json::from_str(&s).unwrap_or_default())
    }

    /// 取项目 → pane 信息
    pub fn get_pane(&self, project_id: &str, selector: &str) -> Result<Option<PaneInfo>> {
        let st = self.read()?;
        Ok(st
            .projects
            .get(project_id)
            .and_then(|p| p.panes.get(selector).cloned()))
    }

    pub fn list_panes(&self, project_id: &str) -> Result<Vec<(String, PaneInfo)>> {
        let st = self.read()?;
        Ok(st
            .projects
            .get(project_id)
            .map(|p| p.panes.iter().map(|(s, i)| (s.clone(), i.clone())).collect())
            .unwrap_or_default())
    }

    pub fn project_state(&self, project_id: &str) -> Result<Option<ProjectState>> {
        Ok(self.read()?.projects.get(project_id).cloned())
    }

    /// 锁内: 读 → 修改 → 原子写
    fn modify<F: FnOnce(&mut Value)>(&self, f: F) -> Result<()> {
        // 父目录的 .lock 文件做 file lock
        let lock_path = self.path.with_file_name(".panes.lock");
        let lock = OpenOptions::new()
            .create(true)
            .write(true)
            .read(true)
            .truncate(false)
            .open(&lock_path)?;
        lock.lock_exclusive()?;

        // 读
        let mut value: Value = if self.path.exists() {
            let mut f = File::open(&self.path)?;
            let mut buf = String::new();
            f.read_to_string(&mut buf)?;
            if buf.trim().is_empty() {
                Value::Object(Map::new())
            } else {
                serde_json::from_str(&buf).unwrap_or(Value::Object(Map::new()))
            }
        } else {
            Value::Object(Map::new())
        };

        f(&mut value);

        // 原子写: tmp + rename
        let tmp = self.path.with_extension("json.tmp");
        {
            let mut tf = File::create(&tmp)?;
            let s = serde_json::to_string_pretty(&value)?;
            tf.write_all(s.as_bytes())?;
            tf.sync_all()?;
        }
        std::fs::rename(&tmp, &self.path)?;

        let _ = FileExt::unlock(&lock);
        Ok(())
    }

    pub fn set_window(&self, project_id: &str, window_id: u64) -> Result<()> {
        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
        self.modify(|v| {
            let obj = v.as_object_mut().unwrap();
            let proj = obj
                .entry(project_id.to_string())
                .or_insert_with(|| serde_json::json!({"panes": {}, "started_at": now.clone()}));
            let proj = proj.as_object_mut().unwrap();
            proj.insert("wezterm_window_id".into(), Value::from(window_id));
            proj.entry("started_at".to_string())
                .or_insert(Value::from(now.clone()));
            proj.entry("panes".to_string())
                .or_insert(Value::Object(Map::new()));
        })
    }

    pub fn add_pane(
        &self,
        project_id: &str,
        selector: &str,
        pane_id: u64,
        session_id: &str,
    ) -> Result<()> {
        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
        self.modify(|v| {
            let obj = v.as_object_mut().unwrap();
            let proj = obj
                .entry(project_id.to_string())
                .or_insert_with(|| serde_json::json!({"panes": {}, "started_at": now.clone()}));
            let proj = proj.as_object_mut().unwrap();
            let panes = proj
                .entry("panes".to_string())
                .or_insert(Value::Object(Map::new()))
                .as_object_mut()
                .unwrap();
            panes.insert(
                selector.to_string(),
                serde_json::json!({
                    "pane_id": pane_id,
                    "session_id": session_id,
                    "started_at": now,
                }),
            );
        })
    }

    pub fn remove_pane(&self, project_id: &str, selector: &str) -> Result<()> {
        self.modify(|v| {
            if let Some(proj) = v.get_mut(project_id).and_then(|p| p.as_object_mut()) {
                if let Some(panes) = proj.get_mut("panes").and_then(|p| p.as_object_mut()) {
                    panes.remove(selector);
                }
            }
        })
    }
}

/// 项目根: $SSHOPS_PROJECT 或 pwd -P
pub fn project_id() -> String {
    if let Ok(p) = std::env::var("SSHOPS_PROJECT") {
        return p;
    }
    std::env::current_dir()
        .ok()
        .and_then(|p| p.canonicalize().ok())
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| ".".into())
}

/// project_slug: basename(project_id), 文件名安全, 截 64 字符
pub fn project_slug() -> String {
    let id = project_id();
    let base = Path::new(&id)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project");
    base.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .take(64)
        .collect()
}

/// 抑制 fs2::FileExt 的 unused warning
#[allow(dead_code)]
fn _force_seek<F: Seek>(_f: &mut F) {}
