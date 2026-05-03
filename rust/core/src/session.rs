//! 会话执行: 注入命令 + 等 prompt + 切片输出 (核心业务逻辑)
//!
//! 这是 Phase B/C 共用的引擎.
//! Phase B 用 SyncExecutor 包装它 (短命 binary).
//! Phase C 用 daemon 持久化包装 (复用此引擎).

use crate::cast_client::CastClient;
use crate::wezterm_mux::WezTermClient;
use crate::{ExecuteRequest, ExecuteResponse, Result};
use std::path::Path;
use std::time::{Duration, Instant};

/// 在指定 pane 注入命令, 通过 cast.sock 等响应
pub async fn execute(
    wez: &WezTermClient,
    pane_id: u64,
    sock_path: &Path,
    req: &ExecuteRequest,
    session_id: &str,
) -> Result<ExecuteResponse> {
    let start = Instant::now();

    // 1. 连接 cast.sock (cast-recorder 已启动, sock 已 ready)
    let mut cast = CastClient::connect(sock_path).await?;

    // 2. 注入命令 (wezterm cli send-text 加 \r)
    let cmd_with_cr = format!("{}\r", req.cmd);
    wez.send_text(pane_id, &cmd_with_cr)?;

    // 3. 阻塞读 cast.sock 直到 prompt 出现
    let timeout = Duration::from_millis(req.timeout_ms);
    let raw_bytes = cast.read_until_prompt(timeout).await?;

    // 4. 切片: strip ANSI + 去掉首行 (命令 echo) + 末行 (prompt)
    let stripped = String::from_utf8_lossy(&raw_bytes);
    let cleaned = strip_ansi(&stripped);
    let mut lines: Vec<&str> = cleaned.lines().collect();
    if !lines.is_empty() {
        lines.remove(0);
    }
    if let Some(last) = lines.last() {
        if last.ends_with("# ") || last.ends_with("$ ") {
            lines.pop();
        }
    }
    let output = lines.join("\n");

    let duration_ms = start.elapsed().as_millis() as u64;

    Ok(ExecuteResponse {
        exit: 0,
        output,
        duration_ms,
        cast_offset: 0.0, // TODO: 算
        session_id: session_id.into(),
        selector: req.selector.clone(),
        dangerous: false,
        blocked: false,
        recent_human_activity: vec![],
    })
}

fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if let Some(&next) = chars.peek() {
                if next == '[' {
                    chars.next();
                    while let Some(c) = chars.next() {
                        if c.is_ascii_alphabetic() {
                            break;
                        }
                    }
                    continue;
                }
                if next == ']' {
                    chars.next();
                    while let Some(c) = chars.next() {
                        if c == '\x07' {
                            break;
                        }
                    }
                    continue;
                }
            }
        }
        out.push(c);
    }
    out
}