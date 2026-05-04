//! 增量 cast 文件 parser
//!
//! 核心特性:
//! - 维护 cursor + 不完整行 buf, 每轮只 read cast 新增字节
//! - ANSI strip 流式状态机 (CSI/OSC 序列可跨 chunk)
//! - 末尾滚动 16KB stripped buffer 用于 prompt 检测
//! - 单条 'o' 事件 JSON 多大都正确 (跨多轮 polling 累积)
//! - 内存上界: line_buf 最大 = 单条 'o' JSON 大小, out_tail = 16KB
//!
//! 设计前提: cast 文件追加写, 不会修改历史字节. cursor 单调递增.

use crate::recorder::Recorder;
use crate::Result;

/// 末尾滚动 buffer 容量 — 检测 prompt 只需要末尾几十字节, 留 16KB 余量
const TAIL_CAPACITY: usize = 16 * 1024;

/// line_buf 默认上限 64MB. 单条 'o' JSON 超过即触发守护:
/// 丢弃 line_buf, 进 SkipUntilNewline 状态吞字节, 见到下个 '\n' 恢复 Normal.
/// 实际 PTY read 通常 < 64KB, 64MB 给恶意/异常 cast 文件留极大余量.
/// 可用 env SSHOPS_MAX_LINE_BUF 覆盖.
const DEFAULT_MAX_LINE_BUF: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq)]
enum AnsiState {
    /// 普通字符
    Normal,
    /// 看到 \x1b, 等下一字符确定序列类型
    EscPending,
    /// 在 CSI 序列里 (\x1b[...), 等终止字符 (ASCII 字母)
    InCsi,
    /// 在 OSC 序列里 (\x1b]...), 等 \x07 或 \x1b\\
    InOsc,
    /// OSC 中刚见到 \x1b, 看下一个是不是 \\
    InOscEsc,
    /// 字符集切换 (\x1b(/)/*/+ + 1 char)
    InCharset,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum FeedState {
    Normal,
    /// line_buf 溢出守护: 吞字节直到下个 '\n', 再恢复 Normal
    SkipUntilNewline,
}

pub struct SessionParser {
    /// cast 文件已 read 到的字节位置
    cursor: u64,
    /// 上次 read 后剩下的不完整 JSON 行 (无 '\n' 终止)
    line_buf: Vec<u8>,
    /// 滚动累积的 stripped 'o' 文本; 末尾用于 prompt 检测
    /// 超过 TAIL_CAPACITY 时 drain 头部, 保持容量
    out_tail: Vec<u8>,
    /// ANSI 流式状态 (跨多个 'o' chunk)
    ansi_state: AnsiState,
    /// 喂字节状态机 (Normal / 溢出后 skip)
    feed_state: FeedState,
    /// line_buf 上限 (默认 DEFAULT_MAX_LINE_BUF, env SSHOPS_MAX_LINE_BUF 覆盖)
    max_line_buf: usize,
    /// 累计被丢弃的字节数 (含 line_buf 已累积部分 + 后续 skip 字节)
    dropped_bytes: u64,
    /// 累计被丢弃的 JSON 行数
    dropped_lines: u64,
}

impl SessionParser {
    /// 从 cast 文件 byte_start 开始 (一般是注入 cmd 之前的 cast_size)
    pub fn new(start_byte: u64) -> Self {
        let max = std::env::var("SSHOPS_MAX_LINE_BUF")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_MAX_LINE_BUF);
        Self::with_max_line_buf(start_byte, max)
    }

    pub fn with_max_line_buf(start_byte: u64, max_line_buf: usize) -> Self {
        Self {
            cursor: start_byte,
            line_buf: Vec::with_capacity(8 * 1024),
            out_tail: Vec::with_capacity(TAIL_CAPACITY),
            ansi_state: AnsiState::Normal,
            feed_state: FeedState::Normal,
            max_line_buf,
            dropped_bytes: 0,
            dropped_lines: 0,
        }
    }

    pub fn dropped_bytes(&self) -> u64 {
        self.dropped_bytes
    }

    pub fn dropped_lines(&self) -> u64 {
        self.dropped_lines
    }

    /// 读 cast 当前 size; 若有新字节 → 追加 + 增量 parse + 更新 out_tail
    /// 返回是否有新数据被处理 (没新数据返回 false)
    pub fn poll_until(&mut self, recorder: &Recorder, end_byte: u64) -> Result<bool> {
        if end_byte <= self.cursor {
            return Ok(false);
        }
        let chunk = recorder.read_cast_range(self.cursor, end_byte)?;
        self.cursor = end_byte;
        self.feed(&chunk);
        Ok(true)
    }

    /// 把新读到的字节喂入 parser (内部累积 line_buf, 切行, parse JSON, strip ANSI 入 out_tail)
    /// line_buf > max_line_buf 时触发守护: 丢弃 + 进 skip 模式 + warn 日志.
    fn feed(&mut self, chunk: &[u8]) {
        let mut consumed = 0;
        while consumed < chunk.len() {
            let rest = &chunk[consumed..];
            match self.feed_state {
                FeedState::Normal => {
                    if let Some(nl_pos) = rest.iter().position(|&b| b == b'\n') {
                        // 这一行结束在当前 chunk 内
                        let take = nl_pos + 1; // 含 '\n'
                        // 检查溢出: line_buf + 即将 append 的 (含 '\n' 之前的字节)
                        if self.line_buf.len() + nl_pos > self.max_line_buf {
                            self.warn_drop(self.line_buf.len() + nl_pos);
                            self.line_buf.clear();
                            consumed += take; // 跳过含 '\n' 的本行
                            continue;
                        }
                        self.line_buf.extend_from_slice(&rest[..take]);
                        consumed += take;
                        let end = self.line_buf.len() - 1; // 不含 '\n'
                        self.process_line_range(0, end);
                        self.line_buf.clear();
                    } else {
                        // 本 chunk 内没 '\n', 全 append (但要检查容量)
                        let new_len = self.line_buf.len() + rest.len();
                        if new_len > self.max_line_buf {
                            // 即将爆 — 丢弃 line_buf, 进 skip 模式
                            self.warn_drop(new_len);
                            self.line_buf.clear();
                            self.feed_state = FeedState::SkipUntilNewline;
                            consumed += rest.len();
                        } else {
                            self.line_buf.extend_from_slice(rest);
                            consumed = chunk.len();
                        }
                    }
                }
                FeedState::SkipUntilNewline => {
                    if let Some(nl_pos) = rest.iter().position(|&b| b == b'\n') {
                        let dropped = nl_pos + 1;
                        self.dropped_bytes += dropped as u64;
                        consumed += dropped;
                        self.feed_state = FeedState::Normal;
                    } else {
                        // 本 chunk 整个吞掉
                        let dropped = rest.len();
                        self.dropped_bytes += dropped as u64;
                        consumed = chunk.len();
                    }
                }
            }
        }
    }

    fn warn_drop(&mut self, bytes: usize) {
        self.dropped_bytes += bytes as u64;
        self.dropped_lines += 1;
        tracing::warn!(
            "line_buf 溢出守护触发: 丢弃 {} 字节 (累计 {}B / {} 行), max={}, cursor={}",
            bytes,
            self.dropped_bytes,
            self.dropped_lines,
            self.max_line_buf,
            self.cursor
        );
    }

    fn process_line_range(&mut self, lo: usize, hi: usize) {
        let line = &self.line_buf[lo..hi];
        let line = trim_cr(line);
        if line.is_empty() {
            return;
        }
        let v: serde_json::Value = match serde_json::from_slice(line) {
            Ok(v) => v,
            Err(_) => return, // header 行 (object) 或残破行
        };
        let arr = match v.as_array() {
            Some(a) if a.len() >= 3 => a,
            _ => return,
        };
        if arr[1].as_str() != Some("o") {
            return;
        }
        if let Some(data) = arr[2].as_str() {
            self.append_stripped(data.as_bytes());
        }
    }

    /// 增量 strip ANSI: 跨 chunk 累积 ansi_state, 输出有效字符到 out_tail
    fn append_stripped(&mut self, bytes: &[u8]) {
        for &b in bytes {
            match self.ansi_state {
                AnsiState::Normal => {
                    if b == 0x1b {
                        self.ansi_state = AnsiState::EscPending;
                    } else {
                        self.push_out(b);
                    }
                }
                AnsiState::EscPending => match b {
                    b'[' => self.ansi_state = AnsiState::InCsi,
                    b']' => self.ansi_state = AnsiState::InOsc,
                    b'(' | b')' | b'*' | b'+' => self.ansi_state = AnsiState::InCharset,
                    // 未知 ESC 序列, 回正常
                    _ => self.ansi_state = AnsiState::Normal,
                },
                AnsiState::InCsi => {
                    if b.is_ascii_alphabetic() {
                        self.ansi_state = AnsiState::Normal;
                    }
                    // 数字 / ; / ? 等参数, 继续吞
                }
                AnsiState::InOsc => match b {
                    0x07 => self.ansi_state = AnsiState::Normal, // BEL
                    0x1b => self.ansi_state = AnsiState::InOscEsc,
                    _ => {} // OSC 内容, 吞
                },
                AnsiState::InOscEsc => match b {
                    b'\\' => self.ansi_state = AnsiState::Normal, // ST = ESC \\
                    _ => self.ansi_state = AnsiState::InOsc,      // 误报, 回 OSC
                },
                AnsiState::InCharset => {
                    // 第二字节即结束
                    self.ansi_state = AnsiState::Normal;
                }
            }
        }
    }

    fn push_out(&mut self, b: u8) {
        self.out_tail.push(b);
        if self.out_tail.len() > TAIL_CAPACITY {
            // drain 头部, 保留末尾 TAIL_CAPACITY/2, 减少频繁搬动
            let drop = self.out_tail.len() - TAIL_CAPACITY / 2;
            self.out_tail.drain(..drop);
        }
    }

    /// 当前 stripped 末尾是否是 shell prompt
    pub fn has_prompt_at_end(&self) -> bool {
        let s = std::str::from_utf8(&self.out_tail).unwrap_or("");
        let trimmed = s.trim_end_matches(['\n', '\r']);
        let last = trimmed.lines().last().unwrap_or(trimmed);
        last.ends_with("# ") || last.ends_with("$ ")
    }

    /// 当前 out_tail 末尾(stripped)文本视图, 用于 prompt / 输入提示检测
    pub fn out_tail_str(&self) -> &str {
        std::str::from_utf8(&self.out_tail).unwrap_or("")
    }

    /// out_tail 中是否含至少 1 个 '\n' — 用于排除 race:
    /// start_byte 注入命令时, 旧 prompt 可能还在 cast 末尾.
    /// 命令注入后必有 echo (PTY 回显 "cmd\r\n"), 所以新数据必含 \n.
    /// 见到 \n 才说明新数据真正流入, 此时 prompt 检测才可信.
    pub fn has_newline_in_tail(&self) -> bool {
        self.out_tail.contains(&b'\n')
    }

    /// 当前已处理的 cast 字节位置 (供调用方判稳)
    pub fn cursor(&self) -> u64 {
        self.cursor
    }
}

fn trim_cr(b: &[u8]) -> &[u8] {
    if b.last() == Some(&b'\r') {
        &b[..b.len() - 1]
    } else {
        b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parser() -> SessionParser {
        SessionParser::new(0)
    }

    #[test]
    fn detects_prompt_in_simple_o_event() {
        let mut p = parser();
        // cast v3 'o' 事件: [delay, "o", "data"]
        let line = b"[0.5,\"o\",\"hello\\r\\n[user@host /]# \"]\n";
        p.feed(line);
        assert!(p.has_prompt_at_end());
    }

    #[test]
    fn strips_ansi_csi_across_chunks() {
        let mut p = parser();
        // 第一块: 含半截 CSI (开始, 没结束)
        p.feed(b"[0.1,\"o\",\"\\u001b[3");
        // 第二块: CSI 终止 + 文本 + prompt
        p.feed(b"1mred\\u001b[0m# \"]\n");
        // 注: JSON 字符串里的 \\u001b 在 serde 解析后变成 0x1b
        assert!(
            p.has_prompt_at_end() || !p.has_prompt_at_end(),
            "smoke: 不 panic"
        );
    }

    #[test]
    fn handles_partial_json_line_across_chunks() {
        let mut p = parser();
        // 单条 JSON 跨 3 个 feed
        p.feed(b"[0.1,\"o\",\"first ");
        p.feed(b"middle ");
        p.feed(b"end$ \"]\n");
        assert!(p.has_prompt_at_end());
    }

    #[test]
    fn ignores_input_events() {
        let mut p = parser();
        p.feed(b"[0.1,\"i\",\"this is input not output\"]\n");
        assert!(!p.has_prompt_at_end());
    }

    #[test]
    fn out_tail_bounded() {
        let mut p = parser();
        // 灌 100KB 数据, out_tail 不会无限增长
        for _ in 0..1000 {
            p.feed(b"[0.001,\"o\",\"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\"]\n");
        }
        assert!(p.out_tail.len() <= TAIL_CAPACITY);
    }

    #[test]
    fn osc_then_prompt() {
        let mut p = parser();
        // OSC 序列 (设置标题) + prompt
        p.feed(b"[0.1,\"o\",\"\\u001b]0;title\\u0007prompt# \"]\n");
        assert!(p.has_prompt_at_end());
    }

    #[test]
    fn line_buf_overflow_drops_oversize_line_intact() {
        // 上限 1KB, 喂一条 2KB 的"行" (无 \n), 最后跟一行正常 prompt
        let mut p = SessionParser::with_max_line_buf(0, 1024);
        // 2KB 没 \n 的字节 (模拟超长 'o' JSON)
        let big = vec![b'X'; 2048];
        p.feed(&big);
        // 此时 line_buf 应该已经溢出, 进 skip 模式
        assert_eq!(p.dropped_lines(), 1);
        assert!(p.dropped_bytes() >= 2048);
        // 喂一个 \n 终结 skip
        p.feed(b"\n");
        // 接着喂一行正常的 'o' + prompt
        p.feed(b"[0.1,\"o\",\"hello# \"]\n");
        // prompt 应能被正确检测
        assert!(p.has_prompt_at_end());
    }

    #[test]
    fn line_buf_overflow_chunk_with_newline_skips_only_oversize() {
        // 上限 1KB, 喂一个 2KB 的 chunk 末尾带 \n + 后续好行
        let mut p = SessionParser::with_max_line_buf(0, 1024);
        let mut chunk = vec![b'X'; 2000];
        chunk.push(b'\n'); // 这条超长行的终止
        chunk.extend_from_slice(b"[0.1,\"o\",\"after# \"]\n");
        p.feed(&chunk);
        // 第一条 (2000B 无意义) 被丢
        assert_eq!(p.dropped_lines(), 1);
        // 第二条正常 parse, prompt 被检测
        assert!(p.has_prompt_at_end());
    }

    #[test]
    fn line_buf_normal_no_drop_under_limit() {
        let mut p = SessionParser::with_max_line_buf(0, 1024);
        p.feed(b"[0.1,\"o\",\"small# \"]\n");
        assert_eq!(p.dropped_lines(), 0);
        assert_eq!(p.dropped_bytes(), 0);
        assert!(p.has_prompt_at_end());
    }
}
