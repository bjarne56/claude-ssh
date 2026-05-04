//! 持久化 pane 状态: SSHOPS_HOME/state/panes.json
//!
//! 新 schema (sessions 中间层, 区分多 Claude 实例):
//! ```json
//! {
//!   "<project_id>": {
//!     "sessions": {
//!       "<session_key>": {              // session_key = WEZTERM_PANE 等
//!         "wezterm_window_id": 17,
//!         "started_at": "...",
//!         "panes": { "<selector>": { pane_id, session_id, started_at } }
//!       }
//!     }
//!   }
//! }
//! ```
//!
//! 老 schema 自动迁移到 sessions["default"] 下 (load 时透明转换).

use crate::Result;
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};

/// 默认 session key (无 WEZTERM_PANE 也无显式覆盖时用)
pub const DEFAULT_SESSION_KEY: &str = "default";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaneInfo {
    pub pane_id: u64,
    pub session_id: String,
    pub started_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionState {
    #[serde(default)]
    pub wezterm_window_id: u64,
    #[serde(default)]
    pub started_at: String,
    #[serde(default)]
    pub panes: HashMap<String, PaneInfo>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectState {
    /// per-Claude session: key = session_key (WEZTERM_PANE 等)
    #[serde(default)]
    pub sessions: HashMap<String, SessionState>,
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
        // 兼容老 schema: 项目对象顶层有 wezterm_window_id 和 panes (没有 sessions)
        let raw: Value = serde_json::from_str(&s).unwrap_or(Value::Object(Map::new()));
        Ok(parse_panes_state(raw))
    }

    /// 取项目 → session → pane 信息
    pub fn get_pane(
        &self,
        project_id: &str,
        session_key: &str,
        selector: &str,
    ) -> Result<Option<PaneInfo>> {
        let st = self.read()?;
        Ok(st
            .projects
            .get(project_id)
            .and_then(|p| p.sessions.get(session_key))
            .and_then(|s| s.panes.get(selector).cloned()))
    }

    pub fn list_panes(
        &self,
        project_id: &str,
        session_key: &str,
    ) -> Result<Vec<(String, PaneInfo)>> {
        let st = self.read()?;
        Ok(st
            .projects
            .get(project_id)
            .and_then(|p| p.sessions.get(session_key))
            .map(|s| s.panes.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default())
    }

    /// 列项目所有 session 的 pane (用于 list-panes --all)
    pub fn list_panes_all(
        &self,
        project_id: &str,
    ) -> Result<HashMap<String, Vec<(String, PaneInfo)>>> {
        let st = self.read()?;
        Ok(st
            .projects
            .get(project_id)
            .map(|p| {
                p.sessions
                    .iter()
                    .map(|(k, s)| {
                        (
                            k.clone(),
                            s.panes
                                .iter()
                                .map(|(sel, info)| (sel.clone(), info.clone()))
                                .collect(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    pub fn session_state(
        &self,
        project_id: &str,
        session_key: &str,
    ) -> Result<Option<SessionState>> {
        Ok(self
            .read()?
            .projects
            .get(project_id)
            .and_then(|p| p.sessions.get(session_key).cloned()))
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

    pub fn set_window(
        &self,
        project_id: &str,
        session_key: &str,
        window_id: u64,
    ) -> Result<()> {
        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
        let pid = project_id.to_string();
        let sk = session_key.to_string();
        self.modify(move |v| {
            // 先 migrate 老格式
            migrate_in_place(v);
            let obj = v.as_object_mut().unwrap();
            let proj = obj
                .entry(pid)
                .or_insert_with(|| serde_json::json!({"sessions": {}}));
            let sessions = proj
                .as_object_mut()
                .unwrap()
                .entry("sessions".to_string())
                .or_insert(Value::Object(Map::new()))
                .as_object_mut()
                .unwrap();
            let sess = sessions
                .entry(sk)
                .or_insert_with(|| serde_json::json!({"panes": {}, "started_at": now.clone()}));
            let sess = sess.as_object_mut().unwrap();
            sess.insert("wezterm_window_id".into(), Value::from(window_id));
            sess.entry("started_at".to_string())
                .or_insert(Value::from(now.clone()));
            sess.entry("panes".to_string())
                .or_insert(Value::Object(Map::new()));
        })
    }

    pub fn add_pane(
        &self,
        project_id: &str,
        session_key: &str,
        selector: &str,
        pane_id: u64,
        session_id: &str,
    ) -> Result<()> {
        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
        let pid = project_id.to_string();
        let sk = session_key.to_string();
        let sel = selector.to_string();
        let sid = session_id.to_string();
        self.modify(move |v| {
            migrate_in_place(v);
            let obj = v.as_object_mut().unwrap();
            let proj = obj
                .entry(pid)
                .or_insert_with(|| serde_json::json!({"sessions": {}}));
            let sessions = proj
                .as_object_mut()
                .unwrap()
                .entry("sessions".to_string())
                .or_insert(Value::Object(Map::new()))
                .as_object_mut()
                .unwrap();
            let sess = sessions
                .entry(sk)
                .or_insert_with(|| serde_json::json!({"panes": {}, "started_at": now.clone()}));
            let panes = sess
                .as_object_mut()
                .unwrap()
                .entry("panes".to_string())
                .or_insert(Value::Object(Map::new()))
                .as_object_mut()
                .unwrap();
            panes.insert(
                sel,
                serde_json::json!({
                    "pane_id": pane_id,
                    "session_id": sid,
                    "started_at": now,
                }),
            );
        })
    }

    pub fn remove_pane(
        &self,
        project_id: &str,
        session_key: &str,
        selector: &str,
    ) -> Result<()> {
        let pid = project_id.to_string();
        let sk = session_key.to_string();
        let sel = selector.to_string();
        self.modify(move |v| {
            migrate_in_place(v);
            if let Some(proj) = v.get_mut(&pid).and_then(|p| p.as_object_mut()) {
                if let Some(sessions) = proj.get_mut("sessions").and_then(|s| s.as_object_mut()) {
                    if let Some(sess) = sessions.get_mut(&sk).and_then(|s| s.as_object_mut()) {
                        if let Some(panes) = sess.get_mut("panes").and_then(|p| p.as_object_mut()) {
                            panes.remove(&sel);
                        }
                    }
                }
            }
        })
    }
}

/// 把 raw JSON Value 转 PanesState, 自动 migrate 老 schema
fn parse_panes_state(mut raw: Value) -> PanesState {
    migrate_in_place(&mut raw);
    serde_json::from_value(raw).unwrap_or_default()
}

/// 老 schema 在项目对象顶层有 wezterm_window_id + panes (没有 sessions),
/// 把它包到 sessions["default"] 下.
fn migrate_in_place(v: &mut Value) {
    let obj = match v.as_object_mut() {
        Some(o) => o,
        None => return,
    };
    for (_, proj) in obj.iter_mut() {
        let proj_obj = match proj.as_object_mut() {
            Some(o) => o,
            None => continue,
        };
        // 已经是新 schema: 有 sessions 字段
        if proj_obj.contains_key("sessions") {
            continue;
        }
        // 老 schema: 移动 wezterm_window_id / started_at / panes 到 sessions["default"]
        let mut default_session = serde_json::Map::new();
        for k in ["wezterm_window_id", "started_at", "panes"] {
            if let Some(val) = proj_obj.remove(k) {
                default_session.insert(k.to_string(), val);
            }
        }
        if !default_session.is_empty() {
            let mut sessions = serde_json::Map::new();
            sessions.insert(DEFAULT_SESSION_KEY.into(), Value::Object(default_session));
            proj_obj.insert("sessions".into(), Value::Object(sessions));
        }
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

/// session_key: 自动识别"一个 Claude 实例", 同 Claude 多次调用拿同一 key
///
/// 优先级 (从高到低):
///   1. SSHOPS_SESSION_KEY (显式覆盖)
///   2. WEZTERM_PANE (wezterm 内跑 Claude)
///   3. ITERM_SESSION_ID (iTerm2)
///   4. TERM_SESSION_ID (Apple Terminal)
///   5. ssh: SSH_TTY (远程 ssh 跑 Claude, 一个 tty 一个 session)
///   6. fallback: DEFAULT_SESSION_KEY (退回旧行为, 共享 active window)
pub fn current_session_key() -> String {
    if let Ok(k) = std::env::var("SSHOPS_SESSION_KEY") {
        if !k.is_empty() {
            return k;
        }
    }
    let candidates = [
        ("WEZTERM_PANE", "wez"),
        ("ITERM_SESSION_ID", "iterm"),
        ("TERM_SESSION_ID", "term"),
        ("SSH_TTY", "ssh"),
    ];
    for (var, prefix) in candidates {
        if let Ok(v) = std::env::var(var) {
            if !v.is_empty() {
                return format!("{prefix}:{v}");
            }
        }
    }
    DEFAULT_SESSION_KEY.to_string()
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
