//! cast.sock 客户端 — 直接读 cast-recorder 推送的 raw PTY 字节
//!
//! 取代当前 bash 的 tail -c 文件 + jq 解析方式 (省 ~400ms / 调用)
//!
//! 用法:
//!   let client = CastClient::connect(&sock_path).await?;
//!   let until_prompt = client.read_until_prompt(timeout).await?;

use crate::Result;
use std::path::Path;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::net::UnixStream;
use tokio::time::timeout;

pub struct CastClient {
    stream: UnixStream,
    buf: Vec<u8>,
}

impl CastClient {
    pub async fn connect(sock_path: &Path) -> Result<Self> {
        let stream = UnixStream::connect(sock_path).await?;
        Ok(Self {
            stream,
            buf: Vec::with_capacity(64 * 1024),
        })
    }

    /// 阻塞读取, 直到看到 prompt 末尾 (`# ` 或 `$ `) 或超时
    /// 返回累积的全部字节 (含 ANSI)
    pub async fn read_until_prompt(&mut self, max_wait: Duration) -> Result<Vec<u8>> {
        let mut tmp = [0u8; 4096];
        let deadline = tokio::time::Instant::now() + max_wait;

        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                break;
            }
            match timeout(remaining, self.stream.read(&mut tmp)).await {
                Ok(Ok(0)) => break,             // EOF
                Ok(Ok(n)) => {
                    self.buf.extend_from_slice(&tmp[..n]);
                    if has_prompt_at_end(&self.buf) {
                        break;
                    }
                }
                Ok(Err(e)) => return Err(e.into()),
                Err(_) => break, // timeout
            }
        }
        Ok(std::mem::take(&mut self.buf))
    }
}

/// 判断字节流末尾是否是 shell prompt (`# ` 或 `$ `)
fn has_prompt_at_end(buf: &[u8]) -> bool {
    // 取末尾 200 字节, strip ANSI 后检查
    let tail_start = buf.len().saturating_sub(300);
    let tail = &buf[tail_start..];
    let stripped = strip_ansi(tail);
    let s = String::from_utf8_lossy(&stripped);
    s.trim_end_matches(['\n', '\r', ' '])
        .lines()
        .last()
        .map(|line| line.ends_with("# ") || line.ends_with("$ "))
        .unwrap_or(false)
}

/// 极简 ANSI 转义剥除 (只去 CSI 序列, 够检测 prompt)
fn strip_ansi(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len());
    let mut i = 0;
    while i < input.len() {
        if input[i] == 0x1b && i + 1 < input.len() {
            // ESC [ ... letter
            if input[i + 1] == b'[' {
                i += 2;
                while i < input.len() && !(input[i].is_ascii_alphabetic()) {
                    i += 1;
                }
                if i < input.len() {
                    i += 1;
                }
                continue;
            }
            // ESC ] ... BEL
            if input[i + 1] == b']' {
                i += 2;
                while i < input.len() && input[i] != 0x07 {
                    i += 1;
                }
                if i < input.len() {
                    i += 1;
                }
                continue;
            }
        }
        out.push(input[i]);
        i += 1;
    }
    out
}