//! 会话执行: 注入命令 + 等 cast prompt + 切片输出
//!
//! 设计同 bash 版 cmd_run:
//!   1. 记 start_byte = cast 文件大小 (cast-recorder 立即 flush, 字节边界精准)
//!   2. send_text 注入命令
//!   3. 轮询 cast 文件 tail 直到看到 shell prompt (`# ` / `$ `) 出现两次 (稳定)
//!   4. 等 cast flush 稳定后, 从 start_byte 提取所有 'o' 事件拼接, strip ANSI, 去首末行

use crate::recorder::Recorder;
use crate::wezterm_mux::WezTermClient;
use crate::Result;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct ExecuteOutcome {
    pub output: String,
    pub exit: i32,
    pub duration_ms: u64,
    pub cast_offset: f64,
    pub start_byte: u64,
    pub end_byte: u64,
    pub timed_out: bool,
}

pub fn execute(
    wez: &WezTermClient,
    pane_id: u64,
    recorder: &Recorder,
    cmd: &str,
    timeout: Duration,
) -> Result<ExecuteOutcome> {
    let t0 = Instant::now();
    let start_byte = recorder.cast_size();

    // 注入命令 (\r 触发执行)
    wez.send_text(pane_id, &format!("{cmd}\r"))?;

    // 等 cast 见 prompt
    let timed_out = !wait_prompt_in_cast(recorder, start_byte, timeout);

    // 等 cast flush 稳定
    recorder.wait_cast_stable(start_byte);

    let end_byte = recorder.cast_size();
    let raw = recorder.extract_output(start_byte, end_byte)?;

    // strip ANSI + 去首行 (命令 echo) + 去末行 (prompt)
    let cleaned = strip_ansi(&raw);
    let mut lines: Vec<&str> = cleaned.lines().collect();
    if !lines.is_empty() {
        lines.remove(0);
    }
    if let Some(last) = lines.last() {
        let t = last.trim_end();
        if t.ends_with("# ") || t.ends_with("$ ") || t.ends_with('#') || t.ends_with('$') {
            lines.pop();
        }
    }
    let output = lines.join("\n");

    Ok(ExecuteOutcome {
        output,
        exit: 0, // bash 版默认 0; 真实退出码无法从屏幕文本可靠还原
        duration_ms: t0.elapsed().as_millis() as u64,
        cast_offset: recorder.cast_offset(),
        start_byte,
        end_byte,
        timed_out,
    })
}

/// 轮询 cast 文件 tail (从 start_byte 起), 直到 prompt 出现两次 (避免 echo 误判) 或超时
/// 返回 true=见到 prompt, false=超时
pub fn wait_prompt_in_cast(recorder: &Recorder, start_byte: u64, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    let poll = Duration::from_millis(50);
    let mut prompt_seen = 0;
    while Instant::now() < deadline {
        let cur = recorder.cast_size();
        if cur > start_byte {
            // 取最近 300 字节 (含足够 'o' 事件文本)
            let read_from = cur.saturating_sub(300).max(start_byte);
            if let Ok(buf) = recorder.read_cast_range(read_from, cur) {
                let text = extract_o_concat(&buf);
                let stripped = strip_ansi(&text);
                let tail: String = stripped
                    .chars()
                    .rev()
                    .take(300)
                    .collect::<String>()
                    .chars()
                    .rev()
                    .collect();
                if has_prompt_at_end(&tail) {
                    prompt_seen += 1;
                    if prompt_seen >= 2 {
                        return true;
                    }
                } else {
                    prompt_seen = 0;
                }
            }
        }
        std::thread::sleep(poll);
    }
    false
}

/// 提取字节区间所有 'o' 事件 data (cast v3 JSONL: [delay, type, data])
fn extract_o_concat(buf: &[u8]) -> String {
    let mut out = String::new();
    for line in buf.split(|&b| b == b'\n') {
        let s = match std::str::from_utf8(line) {
            Ok(s) => s.trim(),
            Err(_) => continue,
        };
        if s.is_empty() {
            continue;
        }
        let v: serde_json::Value = match serde_json::from_str(s) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let arr = match v.as_array() {
            Some(a) if a.len() >= 3 => a,
            _ => continue,
        };
        if arr[1].as_str() != Some("o") {
            continue;
        }
        if let Some(d) = arr[2].as_str() {
            out.push_str(d);
        }
    }
    out
}

fn has_prompt_at_end(s: &str) -> bool {
    // 仅去掉行末 \n / \r, 保留判断需要的空格
    let trimmed = s.trim_end_matches(['\n', '\r']);
    let last = trimmed.lines().last().unwrap_or(trimmed);
    last.ends_with("# ") || last.ends_with("$ ")
}

/// 去除 ANSI CSI/OSC + 字符集切换序列
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
                if matches!(next, '(' | ')' | '*' | '+') {
                    chars.next();
                    chars.next();
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
        let input = "hello\x1b[31mred\x1b[0m world";
        assert_eq!(strip_ansi(input), "hellored world");
    }

    #[test]
    fn strip_ansi_osc() {
        let input = "\x1b]0;title\x07prompt$";
        assert_eq!(strip_ansi(input), "prompt$");
    }

    #[test]
    fn prompt_at_end_hash() {
        assert!(has_prompt_at_end("[user@host /]# "));
    }

    #[test]
    fn prompt_at_end_dollar() {
        assert!(has_prompt_at_end("[user@host ~]$ "));
    }

    #[test]
    fn no_prompt_at_end() {
        assert!(!has_prompt_at_end("hello world"));
    }
}
