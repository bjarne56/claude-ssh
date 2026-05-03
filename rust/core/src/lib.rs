//! ssh-ops core: 共享业务逻辑, 供 bin (Phase B 短命 binary) 和 daemon (Phase C 持久) 复用
//!
//! 设计原则:
//! - 无 fork: wezterm 通过 mux socket 直连, asciinema 通过 cast.sock 直连
//! - 无外部命令依赖: jq/python/awk 全部用 Rust 内存处理
//! - 同步 IO 优先 (短命 binary 不需要 async 复杂度), tokio 仅用于并发场景

pub mod cast_client;
pub mod config;
pub mod error;
pub mod human_detect;
pub mod recorder;
pub mod safety;
pub mod securecrt;
pub mod selector;
pub mod session;
pub mod state;
pub mod wezterm_mux;

pub use error::{Error, Result};

/// Phase B 直接调 session::execute, Phase C 时再加 trait 抽象切 daemon

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