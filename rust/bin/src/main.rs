//! sshops-rs: ssh-ops 命令行工具 (Phase B Rust 重写)
//!
//! 100% 兼容 bash 版的 CLI 接口和 JSON 输出.
//! 短命二进制, 每次 run 启动一次. Phase C 改 daemon 客户端.

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(version, about = "ssh-ops Rust 重写 (Phase B)")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// 在 pane 注入命令并等响应
    Run {
        selector: String,
        cmd: String,
        #[arg(long, default_value_t = 30000)]
        timeout_ms: u64,
    },
    /// 关闭 pane
    Close { selector: String },
    /// 抓取 pane 当前可见文本
    Peek { selector: String },
    /// 列出当前所有 pane
    ListPanes,
    /// 拿最近 N 秒的 human 活动 (供 AI 主动查询)
    Recent {
        selector: String,
        #[arg(long, default_value_t = 60)]
        seconds: u64,
    },
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("SSHOPS_LOG")
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    rt.block_on(async {
        match cli.cmd {
            Cmd::Run { selector, cmd, timeout_ms } => {
                cmd_run(&selector, &cmd, timeout_ms).await
            }
            Cmd::Close { selector } => cmd_close(&selector).await,
            Cmd::Peek { selector } => cmd_peek(&selector).await,
            Cmd::ListPanes => cmd_list_panes().await,
            Cmd::Recent { selector, seconds } => cmd_recent(&selector, seconds).await,
        }
    })
}

async fn cmd_run(_selector: &str, _cmd: &str, _timeout_ms: u64) -> Result<()> {
    // TODO: 完整 cmd_run 实现
    // 1. 解析 selector (SecureCRT 查找)
    // 2. 安全门 (危险命令拦截)
    // 3. pane_open (复用或 spawn)
    // 4. session::execute (注入 + 等 prompt + 切片)
    // 5. 提取 recent_human_activity
    // 6. 输出 JSON
    println!("{{\"todo\": \"cmd_run not yet implemented\"}}");
    Ok(())
}

async fn cmd_close(_selector: &str) -> Result<()> {
    println!("{{\"todo\": \"cmd_close\"}}");
    Ok(())
}

async fn cmd_peek(_selector: &str) -> Result<()> {
    println!("{{\"todo\": \"cmd_peek\"}}");
    Ok(())
}

async fn cmd_list_panes() -> Result<()> {
    println!("{{\"todo\": \"cmd_list_panes\"}}");
    Ok(())
}

async fn cmd_recent(_selector: &str, _seconds: u64) -> Result<()> {
    println!("{{\"todo\": \"cmd_recent\"}}");
    Ok(())
}