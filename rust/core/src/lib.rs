//! ssh-ops core: 共享业务逻辑, 供 bin (Phase B 短命 binary) 和 daemon (Phase C 持久) 复用
//!
//! 设计原则:
//! - 无 fork: wezterm 通过 mux socket 直连 (Phase C), Phase B 暂用 wezterm cli
//! - 无外部命令依赖: jq/python/awk 全部用 Rust 内存处理
//! - 同步 IO 优先 (短命 binary 不需要 async 复杂度), tokio 仅用于 cast.sock

pub mod cast_client;
pub mod config;
pub mod error;
pub mod human_detect;
pub mod ipc;
pub mod pane;
pub mod recorder;
pub mod safety;
pub mod securecrt;
pub mod selector;
pub mod session;
pub mod state;
pub mod wezterm_mux;

pub use error::{Error, Result};

/// SSHOPS_HOME: 环境变量优先, 否则取 binary 上一级目录
pub fn sshops_home() -> std::path::PathBuf {
    if let Ok(p) = std::env::var("SSHOPS_HOME") {
        return std::path::PathBuf::from(p);
    }
    // bin 路径: <SSHOPS_HOME>/rust/target/release/sshops-rs 或 <SSHOPS_HOME>/bin/sshops
    // fallback: 当前目录
    std::env::current_exe()
        .ok()
        .and_then(|p| {
            // 朝上找 bin/cast-recorder 存在的目录
            let mut cur = p.parent()?.to_path_buf();
            for _ in 0..6 {
                if cur.join("bin/cast-recorder").exists() {
                    return Some(cur);
                }
                cur = cur.parent()?.to_path_buf();
            }
            None
        })
        .unwrap_or_else(|| std::path::PathBuf::from("."))
}

/// SSHOPS_HOME/state
pub fn state_dir(home: &std::path::Path) -> std::path::PathBuf {
    home.join("state")
}

/// 命令注入请求 (Phase C 时也是 IPC 消息)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExecuteRequest {
    pub selector: String,
    pub cmd: String,
    pub timeout_ms: u64,
    pub auto_human: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExecuteResponse {
    pub exit: i32,
    pub output: String,
    pub duration_ms: u64,
    pub cast_offset: f64,
    pub session_id: String,
    pub selector: String,
    pub dangerous: bool,
    pub blocked: bool,
    pub recent_human_activity: Vec<human_detect::HumanCmd>,
}
