//! IPC 协议: sshops-rs (cli) ↔ sshops-daemon
//!
//! Wire format: 长度前缀 bincode
//!   [u32 BE len][bincode payload]
//!
//! 设计原则:
//! - Request/Response 严格 1:1, 不做 streaming (Phase D 再加)
//! - 所有外部状态 (selector, project_id, env) 都显式带在 request 里, daemon 不读 cli 的环境

use crate::human_detect::HumanCmd;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// 协议版本: 不兼容时 daemon 拒绝旧 cli
pub const PROTO_VERSION: u32 = 2;

/// 默认 socket 路径: $SSHOPS_HOME/state/daemon.sock
pub fn default_sock_path(sshops_home: &std::path::Path) -> PathBuf {
    sshops_home.join("state/daemon.sock")
}

/// pid 文件: 单实例 + auto-spawn 校验
pub fn default_pid_path(sshops_home: &std::path::Path) -> PathBuf {
    sshops_home.join("state/daemon.pid")
}

/// daemon 日志: 后台进程 stderr 写到这里
pub fn default_log_path(sshops_home: &std::path::Path) -> PathBuf {
    sshops_home.join("state/daemon.log")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientCtx {
    /// 客户端的 SSHOPS_HOME (daemon 启动时已知, 这里只用作 sanity check)
    pub sshops_home: PathBuf,
    /// 客户端 pwd, 作为 project_id (跟 state.rs::project_id 一致)
    pub project_id: String,
    /// 客户端协议版本
    pub proto: u32,
    /// session_key: 区分多 Claude 实例 (优先 SSHOPS_SESSION_KEY > WEZTERM_PANE > "default")
    /// 同 session 共用 wezterm 窗口 + tab, 不同 session 各自窗口
    #[serde(default = "default_session_key")]
    pub session_key: String,
}

fn default_session_key() -> String {
    "default".to_string()
}

/// IPC 请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IpcRequest {
    Ping,
    /// 优雅停 daemon (flush state + exit)
    Shutdown,
    /// 获取 daemon 状态 (uptime / pane 数量 / mem)
    Status,
    /// 跑命令
    Run {
        ctx: ClientCtx,
        selector: SelectorSpec,
        cmd: String,
        timeout_ms: u64,
        i_mean_it: bool,
        auto_human: bool,
    },
    /// 仅 spawn pane
    Open {
        ctx: ClientCtx,
        selector: SelectorSpec,
    },
    /// 关 pane
    Close {
        ctx: ClientCtx,
        selector: SelectorSpec,
    },
    /// 取 pane 当前可见文本
    Peek {
        ctx: ClientCtx,
        selector: SelectorSpec,
    },
    /// 列当前项目所有 pane
    ListPanes { ctx: ClientCtx },
    /// 拿最近 N 秒 human 活动
    Recent {
        ctx: ClientCtx,
        selector: SelectorSpec,
        seconds: u64,
    },
    /// 发送原始按键到 pane (不等 prompt 立即返回);
    /// 用于驱动远端交互式菜单 (远端 read -p / dialog 风格).
    /// keys 默认按 escape 序列处理 (\r \n \t \\); raw=true 时字面发送.
    Send {
        ctx: ClientCtx,
        selector: SelectorSpec,
        keys: String,
        raw: bool,
    },
}

/// selector 规格: 复用 cli 端解析后的语义, daemon 不重复解析 .ini 路径
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SelectorSpec {
    /// SecureCRT 入口: @path 或 keyword
    Crt(String),
    /// tmp 入口: 直接 user/host
    Tmp {
        user: String,
        host: String,
        port: u16,
        key: Option<PathBuf>,
        prod: bool,
        auth_type: String,
        password: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IpcResponse {
    Pong,
    Bye,
    Status(StatusInfo),
    Run(RunResp),
    Open(OpenResp),
    Closed,
    Peek(String),
    Panes(PanesResp),
    Recent(Vec<HumanCmd>),
    Send(SendResp),
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendResp {
    pub selector: String,
    pub session_id: String,
    pub bytes_sent: usize,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusInfo {
    pub uptime_secs: u64,
    pub req_count: u64,
    pub pane_count: usize,
    pub session_count: usize,
    pub started_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunResp {
    pub exit: i32,
    pub output: String,
    pub duration_ms: u64,
    pub cast_offset: f64,
    pub session_id: String,
    pub selector: String,
    pub dangerous: bool,
    pub blocked: bool,
    pub reason: Option<String>,
    pub recent_human_activity: Vec<HumanCmd>,
    /// 仅在 pane spawn 时 (cold) 触发: ssh login / sudo 中曾 prompt 用户输入
    #[serde(default)]
    pub password_prompted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenResp {
    pub selector: String,
    pub source: String,
    pub pane_id: u64,
    pub session_id: String,
    pub user: String,
    pub host: String,
    pub port: u16,
    pub key: String,
    pub reused: bool,
    #[serde(default)]
    pub password_prompted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanesResp {
    pub wezterm_window_id: u64,
    pub started_at: String,
    pub panes: Vec<(String, PaneEntry)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaneEntry {
    pub pane_id: u64,
    pub session_id: String,
    pub started_at: String,
}

// === Wire codec ============================================================

/// 写: [u32 BE len][bincode payload]
pub async fn write_msg<W: AsyncWriteExt + Unpin, T: Serialize>(
    w: &mut W,
    msg: &T,
) -> std::io::Result<()> {
    let payload = bincode::serialize(msg).map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, format!("bincode encode: {e}"))
    })?;
    let len = payload.len() as u32;
    if len > 64 * 1024 * 1024 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "msg > 64MB",
        ));
    }
    w.write_all(&len.to_be_bytes()).await?;
    w.write_all(&payload).await?;
    w.flush().await?;
    Ok(())
}

/// 读: [u32 BE len][bincode payload]
pub async fn read_msg<R: AsyncReadExt + Unpin, T: for<'de> Deserialize<'de>>(
    r: &mut R,
) -> std::io::Result<T> {
    let mut len_buf = [0u8; 4];
    r.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > 64 * 1024 * 1024 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "msg > 64MB",
        ));
    }
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf).await?;
    bincode::deserialize(&buf).map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, format!("bincode decode: {e}"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn roundtrip_ping_pong() {
        use tokio::io::duplex;
        let (mut a, mut b) = duplex(1024);
        write_msg(&mut a, &IpcRequest::Ping).await.unwrap();
        let req: IpcRequest = read_msg(&mut b).await.unwrap();
        assert!(matches!(req, IpcRequest::Ping));
    }

    #[tokio::test]
    async fn roundtrip_run() {
        use tokio::io::duplex;
        let (mut a, mut b) = duplex(8192);
        let req = IpcRequest::Run {
            ctx: ClientCtx {
                sshops_home: PathBuf::from("/tmp"),
                project_id: "/p".into(),
                proto: PROTO_VERSION,
                session_key: "wez:7".into(),
            },
            selector: SelectorSpec::Crt("@aws/edge".into()),
            cmd: "uptime".into(),
            timeout_ms: 30000,
            i_mean_it: false,
            auto_human: true,
        };
        write_msg(&mut a, &req).await.unwrap();
        let parsed: IpcRequest = read_msg(&mut b).await.unwrap();
        if let IpcRequest::Run { cmd, .. } = parsed {
            assert_eq!(cmd, "uptime");
        } else {
            panic!("wrong variant");
        }
    }
}
