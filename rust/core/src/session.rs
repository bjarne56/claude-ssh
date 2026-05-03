//! 会话执行: 注入命令 + 等 prompt + 切片输出 (核心引擎)

use crate::cast_client::CastClient;
use crate::wezterm_mux::WezTermClient;
use crate::{Error, ExecuteRequest, ExecuteResponse, Result};
use std::path::Path;
use std::time::{Duration, Instant};

pub async fn execute(
    wez: &WezTermClient,
    pane_id: u64,
    sock_path: &Path,
    req: &ExecuteRequest,
    session_id: &str,
) -> Result<ExecuteResponse> {
    let start = Instant::now();
    let mut cast = CastClient::connect(sock_path)
        .await
        .map_err(|e| Error::Other(format!("connect cast.sock {}: {e}", sock_path.display())))?;

    // 注入命令 (加 \r 触发执行)
    let cmd_with_cr = format!("{}\r", req.cmd);
    wez.send_text(pane_id, &cmd_with_cr)?;

    // 等 prompt
    let timeout = Duration::from_millis(req.timeout_ms);
    let raw_bytes = cast.read_until_prompt(timeout).await?;

    // strip ANSI + 切片
    let stripped = String::from_utf8_lossy(&raw_bytes);
    let cleaned = strip_ansi(&stripped);
    let mut lines: Vec<&str> = cleaned.lines().collect();
    if !lines.is_empty() {
        lines.remove(0); // 命令 echo
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
        cast_offset: 0.0,
        session_id: session_id.into(),
        selector: req.selector.clone(),
        dangerous: false,
        blocked: false,
        recent_human_activity: vec![],
    })
}

/// 去除 ANSI CSI/OSC 序列
pub fn strip_ansi(s: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_ansi_csi() {
        // CSI 序列被剥离, 序列之外文本不变 (含中间空格)
        let input = "hello\x1b[31mred\x1b[0m world";
        assert_eq!(strip_ansi(input), "hellored world");
    }

    #[test]
    fn strip_ansi_osc() {
        let input = "\x1b]0;title\x07prompt$";
        assert_eq!(strip_ansi(input), "prompt$");
    }
}