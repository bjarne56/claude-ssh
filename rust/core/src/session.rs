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
    let timing = std::env::var("SSHOPS_DEBUG_TIMING").as_deref() == Ok("1");
    let log = |name: &str, dt: f64| {
        if timing {
            eprintln!("[TIMING]   ↳ {name:>26}: {dt:>7.2}ms");
        }
    };

    let t0 = Instant::now();
    let start_byte = recorder.cast_size();

    // marker 协议: 命令后追加 echo SSHOPS_END_<nonce>=$?
    // - cast 中出现该字符串 = 命令执行完成 (无需 wait_stable 边界)
    // - exit code 直接从 marker 提取 (修真实 exit code 硬伤)
    let mut nonce_bytes = [0u8; 8];
    use rand::RngCore;
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce: String = nonce_bytes.iter().map(|b| format!("{b:02x}")).collect();
    let marker_prefix = format!("SSHOPS_END_{nonce}=");
    // 打散 marker 字符串避免命令 echo 行含完整 marker 子串导致 wait_for_marker 误判.
    // 命令 echo: `cmd; printf '%s_%s=%d\n' 'SSHOPS' 'END_<nonce>' $?` (无完整 marker_prefix)
    // 执行输出: `SSHOPS_END_<nonce>=0` (有完整 marker_prefix)
    let wrapped = format!(
        "{cmd}; printf '%s_%s=%d\\n' 'SSHOPS' 'END_{nonce}' $?\r"
    );

    // 注入包装后命令
    let t1 = Instant::now();
    wez.send_text(pane_id, &wrapped)?;
    log("send_text", t1.elapsed().as_micros() as f64 / 1000.0);

    // 等 marker 出现
    let t2 = Instant::now();
    let marker_result = wait_for_marker(recorder, start_byte, &marker_prefix, timeout);
    log("wait_for_marker", t2.elapsed().as_micros() as f64 / 1000.0);

    let t4 = Instant::now();
    let end_byte = recorder.cast_size();
    let raw = recorder.extract_output(start_byte, end_byte)?;
    log("extract_output", t4.elapsed().as_micros() as f64 / 1000.0);

    // strip ANSI + 去首行 (命令 echo) + 去 marker 行 + 提取 exit code
    let cleaned = strip_ansi(&raw);
    let mut lines: Vec<&str> = cleaned.lines().collect();
    if !lines.is_empty() {
        lines.remove(0); // 命令 echo 行
    }

    // 找 marker 行, 提取 exit code, 删除该行及之后所有 (含 prompt 残尾)
    let mut exit_code: i32 = if marker_result.is_some() { 0 } else { -1 };
    if let Some((marker_idx, parsed_exit)) = lines
        .iter()
        .enumerate()
        .find_map(|(i, line)| {
            if let Some(rest) = line.find(&marker_prefix).map(|pos| &line[pos + marker_prefix.len()..]) {
                rest.split_whitespace().next().and_then(|s| s.parse::<i32>().ok())
                    .map(|e| (i, e))
            } else {
                None
            }
        })
    {
        exit_code = parsed_exit;
        lines.truncate(marker_idx);
    }
    let output = lines.join("\n");

    Ok(ExecuteOutcome {
        output,
        exit: exit_code,
        duration_ms: t0.elapsed().as_millis() as u64,
        cast_offset: recorder.cast_offset(),
        start_byte,
        end_byte,
        timed_out: marker_result.is_none(),
    })
}

/// 等 cast 末尾出现 marker_prefix (如 "SSHOPS_END_<nonce>=")
/// 返回 Some(()) 见到, None 超时
pub fn wait_for_marker(
    recorder: &Recorder,
    start_byte: u64,
    marker_prefix: &str,
    timeout: Duration,
) -> Option<()> {
    use crate::incremental_parser::SessionParser;
    let deadline = Instant::now() + timeout;
    let poll = Duration::from_millis(10);
    let mut parser = SessionParser::new(start_byte);
    while Instant::now() < deadline {
        let cur = recorder.cast_size();
        if cur > parser.cursor() {
            let _ = parser.poll_until(recorder, cur);
        }
        if parser.out_tail_str().contains(marker_prefix) {
            return Some(());
        }
        std::thread::sleep(poll);
    }
    None
}

/// 紧 polling: 20ms 间隔扫 cast 末尾, 看到 prompt + 1 个无变化窗口即退出
/// 返回 true=见到 prompt 且稳定, false=超时
///
/// 算法:
/// - 每 20ms 取一次 cast_size + tail 检测 prompt
/// - 见到 prompt 后, 记录当前 size; 再等 1 轮 (20ms) 看 size 是否变化
/// - 不变 → 稳定, 退出; 变了 → 重置 (说明 cast 还在继续 flush)
///
/// 比之前 100ms × 3 + 100ms × 3 = 600ms 串行等待快得多 (典型 ~40-80ms)
/// 增量 parser 版本: 维护 SessionParser 持续状态, 永远只 read cast 新增字节.
/// 单条 'o' 事件 JSON 多大都不丢数据 (跨多轮 polling 累积 line_buf).
/// 内存上界: line_buf = 单条 'o' JSON 大小 (完整后被消费), out_tail = 16KB.
pub fn wait_prompt_and_stable(recorder: &Recorder, start_byte: u64, timeout: Duration) -> bool {
    use crate::incremental_parser::SessionParser;
    let deadline = Instant::now() + timeout;
    let poll = Duration::from_millis(10);
    let mut parser = SessionParser::new(start_byte);
    let mut prompt_seen_at_cursor: Option<u64> = None;
    while Instant::now() < deadline {
        let cur = recorder.cast_size();
        if cur > parser.cursor() {
            let _ = parser.poll_until(recorder, cur);
        }
        // 必须 out_tail 含 \n 才说明命令 echo 已流入 (排除 race: start 时旧 prompt 误判)
        if parser.has_newline_in_tail() && parser.has_prompt_at_end() {
            match prompt_seen_at_cursor {
                Some(prev) if prev == parser.cursor() => return true,
                _ => prompt_seen_at_cursor = Some(parser.cursor()),
            }
        } else {
            prompt_seen_at_cursor = None;
        }
        std::thread::sleep(poll);
    }
    false
}

/// 旧版 API 保留兼容
#[deprecated(note = "改用 wait_prompt_and_stable")]
pub fn wait_prompt_in_cast(recorder: &Recorder, start_byte: u64, timeout: Duration) -> bool {
    wait_prompt_and_stable(recorder, start_byte, timeout)
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
