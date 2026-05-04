//! sshops-daemon: 持久后台进程, 接受 cli IPC, 复用所有内存状态
//!
//! 启动: 通常由 cli auto-spawn (C2). 也可手动:
//!   sshops-daemon            # 前台跑, 调试用
//!   sshops-daemon --detach   # fork 到后台, stderr → daemon.log

use anyhow::{Context, Result};
use fs2::FileExt;
use ssh_ops_core::{
    config::{self, Config},
    ipc::{self, IpcRequest},
    securecrt::CrtParser,
    sshops_home, state_dir,
};
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::Mutex;

mod handler;

/// daemon 全局状态 (Arc<Mutex<...>> 包到 handler 里)
pub struct DaemonState {
    pub started_at: Instant,
    pub started_at_iso: String,
    pub req_count: u64,
    pub cfg: Config,
    pub crt_parser: Option<CrtParser>,
    pub home: std::path::PathBuf,
    pub last_req_at: Instant,
    /// per-pane async 锁: key = "{project_id}|{selector}"
    /// 同 pane 的 run/open/close/peek/recent 串行化, 不同 pane 完全并行
    pub pane_locks: std::collections::HashMap<String, std::sync::Arc<tokio::sync::Mutex<()>>>,
}

impl DaemonState {
    fn new(home: std::path::PathBuf, cfg: Config) -> Self {
        let crt_parser = CrtParser::from_config(&cfg).ok();
        Self {
            started_at: Instant::now(),
            started_at_iso: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
            req_count: 0,
            cfg,
            crt_parser,
            home,
            last_req_at: Instant::now(),
            pane_locks: std::collections::HashMap::new(),
        }
    }
}

/// 获取 per-pane 锁 (新建或复用), 同 key 的请求都拿同一个 Mutex
pub async fn get_pane_lock(
    state: &std::sync::Arc<Mutex<DaemonState>>,
    key: &str,
) -> std::sync::Arc<tokio::sync::Mutex<()>> {
    let mut s = state.lock().await;
    s.pane_locks
        .entry(key.to_string())
        .or_insert_with(|| std::sync::Arc::new(tokio::sync::Mutex::new(())))
        .clone()
}

/// 构造 pane lock key
pub fn pane_lock_key(project_id: &str, selector: &str) -> String {
    format!("{project_id}|{selector}")
}

fn parse_args() -> bool {
    // --detach: fork 到后台, stderr → daemon.log
    std::env::args().any(|a| a == "--detach")
}

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> Result<()> {
    let detached = parse_args();

    let home = sshops_home();
    std::fs::create_dir_all(state_dir(&home))?;

    // === 单实例锁: 抢 daemon.pid 文件的 flock, 失败说明已有 daemon ===
    let pid_path = ipc::default_pid_path(&home);
    let pid_file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&pid_path)
        .with_context(|| format!("open pid {}", pid_path.display()))?;
    if FileExt::try_lock_exclusive(&pid_file).is_err() {
        eprintln!("已有 daemon 在跑 (pid file: {})", pid_path.display());
        std::process::exit(0); // 不算错误, 直接退出让 cli 连现有 daemon
    }
    // 成功抢锁 → 写 pid (truncate 后重写)
    let mut pid_file_w = OpenOptions::new().write(true).truncate(true).open(&pid_path)?;
    let _ = pid_file_w.write_all(format!("{}\n", std::process::id()).as_bytes());
    // 注: pid_file 必须保持 open, 否则 flock 自动释放. 用 leak 保活到进程退出.
    Box::leak(Box::new(pid_file));

    // === 日志: detach 时写文件, 否则 stderr ===
    if detached {
        let log_path = ipc::default_log_path(&home);
        let log_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)?;
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_env("SSHOPS_LOG")
                    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
            )
            .with_writer(std::sync::Mutex::new(log_file))
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_env("SSHOPS_LOG")
                    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
            )
            .with_writer(std::io::stderr)
            .init();
    }

    let cfg = config::load(&home)?;
    let sock_path = ipc::default_sock_path(&home);

    // 老 socket 残留清掉 — 这里是 flock 已成功的情况, 必然没人在用
    if sock_path.exists() {
        if let Err(e) = std::fs::remove_file(&sock_path) {
            tracing::warn!("无法清理旧 socket {}: {}", sock_path.display(), e);
        }
    }

    let listener = UnixListener::bind(&sock_path)
        .with_context(|| format!("bind {}", sock_path.display()))?;
    tracing::info!(
        "sshops-daemon 启动: pid={} sock={} detached={detached}",
        std::process::id(),
        sock_path.display()
    );

    let state = Arc::new(Mutex::new(DaemonState::new(home, cfg)));

    // ctrl-c / SIGTERM 优雅退出
    let sock_for_signal = sock_path.clone();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        tracing::info!("收到 SIGINT, 优雅退出...");
        let _ = std::fs::remove_file(&sock_for_signal);
        std::process::exit(0);
    });

    // idle 自动退出: 默认 30 分钟无请求, 用 SSHOPS_DAEMON_IDLE_SECS 覆盖
    let idle_secs: u64 = std::env::var("SSHOPS_DAEMON_IDLE_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(30 * 60);
    let idle_state = state.clone();
    let idle_sock = sock_path.clone();
    tokio::spawn(async move {
        let check_interval = Duration::from_secs(60);
        loop {
            tokio::time::sleep(check_interval).await;
            let elapsed = {
                let s = idle_state.lock().await;
                s.last_req_at.elapsed().as_secs()
            };
            if elapsed >= idle_secs {
                tracing::info!("idle {elapsed}s ≥ {idle_secs}s, 自动退出");
                let _ = std::fs::remove_file(&idle_sock);
                std::process::exit(0);
            }
        }
    });

    loop {
        match listener.accept().await {
            Ok((stream, _addr)) => {
                let state = state.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream, state).await {
                        tracing::warn!("连接处理错: {e}");
                    }
                });
            }
            Err(e) => {
                tracing::error!("accept 失败: {e}");
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }
}

async fn handle_connection(mut stream: UnixStream, state: Arc<Mutex<DaemonState>>) -> Result<()> {
    let req: IpcRequest = ipc::read_msg(&mut stream).await?;
    tracing::debug!("recv: {:?}", req);

    let resp = handler::dispatch(req, state).await;

    ipc::write_msg(&mut stream, &resp).await?;
    Ok(())
}
