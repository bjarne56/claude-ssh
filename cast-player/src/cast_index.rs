// cast_index.rs — asciinema cast 文件解析与索引构建
//
// cast v3 格式: 第一行是 JSON header(单行),后续每行一个 JSON 数组
//   [delay_seconds, event_type, data_string]
//   其中 event_type: "o"=输出, "i"=输入, "x"=退出码
//
// 构建索引: 扫描全文件,累加 delay 得到 elapsed,记录 byte_offset,
// 支持二分查找快速跳转到任意时间点。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CastHeader {
    pub version: u8,
    pub width: Option<u64>,
    pub height: Option<u64>,
    pub term: Option<TermInfo>,
    pub timestamp: Option<u64>,
    pub command: Option<String>,
    pub env: Option<HashMap<String, String>>,
    pub theme: Option<TermTheme>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TermInfo {
    #[serde(default)]
    pub cols: u64,
    #[serde(default)]
    pub rows: u64,
    #[serde(rename = "type", default)]
    pub term_type: String,
    #[serde(default)]
    pub version: String,
    pub theme: Option<TermTheme>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TermTheme {
    pub fg: Option<String>,
    pub bg: Option<String>,
    pub palette: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EventType {
    Output,
    Input,
    Exit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CastEventMeta {
    /// 从录制开始算起的累计秒数
    pub elapsed: f64,
    /// 在 .cast 文件中的字节偏移(line 开头)
    pub byte_offset: u64,
    pub event_type: EventType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CastIndex {
    pub header: CastHeader,
    pub events: Vec<CastEventMeta>,
    pub total_duration: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMeta {
    pub session_id: String,
    pub project: String,
    pub host_resolved: String,
    pub host_selector: String,
    pub user: String,
    pub auth_type: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub command_count: u64,
    pub ai_command_count: u64,
    pub human_command_count: u64,
    pub dangerous_count: u64,
    pub blocked_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandRecord {
    pub ts: String,
    pub actor: String,
    pub host: String,
    pub cmd: String,
    pub exit: i64,
    pub duration_ms: u64,
    pub cast_offset: f64,
    pub dangerous: bool,
    pub blocked: bool,
    pub nonce: String,
    /// 用户开始键入命令第一个字符的 elapsed (用于 seek 起点),
    /// 默认 = max(0, cast_offset - duration_ms/1000 - 3.0) 估算
    #[serde(default)]
    pub input_start_offset: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CastChunk {
    /// 从哪个 elapsed 开始
    pub start_elapsed: f64,
    /// 原始 JSON 行数据
    pub lines: Vec<String>,
}

impl CastIndex {
    /// 扫描整个 .cast 文件构建索引。
    /// 返回 CastIndex 或错误信息。
    pub fn build(cast_path: &Path) -> Result<Self, String> {
        let file = File::open(cast_path).map_err(|e| format!("打开 cast 文件失败: {e}"))?;
        let mut reader = BufReader::new(file);

        // 读 header (第一行)
        let mut header_line = String::new();
        reader
            .read_line(&mut header_line)
            .map_err(|e| format!("读取 header 失败: {e}"))?;
        let header: CastHeader = serde_json::from_str(header_line.trim())
            .map_err(|e| format!("解析 header JSON 失败: {e}"))?;

        let mut events: Vec<CastEventMeta> = Vec::with_capacity(1024);
        let mut elapsed: f64 = 0.0;
        let mut byte_offset: u64 = header_line.len() as u64;

        // 扫描事件行
        let mut line = String::new();
        loop {
            let start = byte_offset;
            line.clear();
            let n = reader
                .read_line(&mut line)
                .map_err(|e| format!("读取事件行失败: {e}"))?;
            if n == 0 {
                break;
            }
            byte_offset += n as u64;

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // 解析 [delay, event_type, data]
            let parts: Vec<serde_json::Value> =
                serde_json::from_str(trimmed).unwrap_or_default();
            if parts.len() < 2 {
                continue;
            }

            let delay: f64 = parts[0].as_f64().unwrap_or(0.0);
            let etype_str = parts[1].as_str().unwrap_or("o");

            elapsed += delay;

            let event_type = match etype_str {
                "i" => EventType::Input,
                "o" => EventType::Output,
                "x" => EventType::Exit,
                _ => EventType::Output,
            };

            events.push(CastEventMeta {
                elapsed,
                byte_offset: start,
                event_type,
            });
        }

        let total_duration = elapsed;

        Ok(CastIndex {
            header,
            events,
            total_duration,
        })
    }

    /// 二分查找: 找到 <= target_elapsed 的最大索引, 带 f64 精度容差
    pub fn find_index_at(&self, target_elapsed: f64) -> usize {
        if self.events.is_empty() {
            return 0;
        }
        let mut lo = 0usize;
        let mut hi = self.events.len() - 1;
        const EPS: f64 = 1e-9;

        while lo < hi {
            let mid = (lo + hi + 1) / 2;
            if self.events[mid].elapsed <= target_elapsed + EPS {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }
        lo
    }

    /// 从已打开的 cast 文件中读取从 start_offset 开始的 raw 事件行,
    /// 直到 elapsed 超过 end_elapsed 或文件结束。
    pub fn read_chunk(
        cast_path: &Path,
        start_byte_offset: u64,
        end_elapsed: Option<f64>,
    ) -> Result<CastChunk, String> {
        let mut file = File::open(cast_path).map_err(|e| format!("打开 cast 文件失败: {e}"))?;
        file.seek(SeekFrom::Start(start_byte_offset))
            .map_err(|e| format!("seek 失败: {e}"))?;

        let reader = BufReader::new(file);
        let mut lines: Vec<String> = Vec::new();
        let mut elapsed: f64 = 0.0;
        let mut first = true;

        for line_result in reader.lines() {
            let line = line_result.map_err(|e| format!("读取行失败: {e}"))?;
            if line.trim().is_empty() {
                if !first {
                    lines.push(line);
                }
                continue;
            }

            if first {
                // 第一条可能是 header,跳过
                first = false;
                if line.trim_start().starts_with('{') {
                    continue;
                }
            }

            // 解析 delay
            let parts: Vec<serde_json::Value> =
                serde_json::from_str(line.trim()).unwrap_or_default();
            if parts.len() >= 2 {
                let delay: f64 = parts[0].as_f64().unwrap_or(0.0);
                elapsed += delay;
            }

            lines.push(line);

            if let Some(end) = end_elapsed {
                if elapsed >= end - start_byte_offset as f64 {
                    break;
                }
            }
        }

        Ok(CastChunk {
            start_elapsed: 0.0, // caller 应自己追踪
            lines,
        })
    }

    /// 读取整个 cast 文件的所有事件行(用于小文件一次性加载)
    pub fn read_all_events(cast_path: &Path) -> Result<Vec<(f64, String)>, String> {
        let file = File::open(cast_path).map_err(|e| format!("打开 cast 文件失败: {e}"))?;
        let reader = BufReader::new(file);
        let mut events: Vec<(f64, String)> = Vec::new();
        let mut first = true;

        for line_result in reader.lines() {
            let line = line_result.map_err(|e| format!("读取行失败: {e}"))?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if first {
                first = false;
                if trimmed.starts_with('{') {
                    continue;
                }
            }
            let parts: Vec<serde_json::Value> =
                serde_json::from_str(trimmed).unwrap_or_default();
            let delay: f64 = if parts.len() >= 2 {
                parts[0].as_f64().unwrap_or(0.0)
            } else {
                0.0
            };
            events.push((delay, line));
        }

        Ok(events)
    }
}

// ---- session meta / commands ----

pub fn load_meta(meta_path: &Path) -> Result<SessionMeta, String> {
    let content =
        std::fs::read_to_string(meta_path).map_err(|e| format!("读取 meta.json 失败: {e}"))?;
    serde_json::from_str(&content).map_err(|e| format!("解析 meta.json 失败: {e}"))
}

pub fn load_commands(commands_path: &Path) -> Result<Vec<CommandRecord>, String> {
    let file =
        File::open(commands_path).map_err(|e| format!("读取 commands.jsonl 失败: {e}"))?;
    let reader = BufReader::new(file);
    let mut records = Vec::new();

    for line_result in reader.lines() {
        let line = line_result.map_err(|e| format!("读取行失败: {e}"))?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let record: CommandRecord =
            serde_json::from_str(trimmed).unwrap_or_else(|_| CommandRecord {
                ts: String::new(),
                actor: String::new(),
                host: String::new(),
                cmd: String::new(),
                exit: -1,
                duration_ms: 0,
                cast_offset: 0.0,
                dangerous: false,
                blocked: false,
                nonce: String::new(),
                input_start_offset: 0.0,
            });
        records.push(record);
    }

    Ok(records)
}

/// 根据 cast_offset 排序 commands,使得二分查找可行
pub fn sort_commands(records: &mut [CommandRecord]) {
    records.sort_by(|a, b| {
        a.cast_offset
            .partial_cmp(&b.cast_offset)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

/// 识别 ssh-ops 在登录后自动注入的初始化命令 (不该归类为 human)
/// 参考 SKILL.md: 登录后立刻注入 sudo -i + PS1 + clear 一条命令
pub fn is_sshops_bootstrap_command(cmd: &str) -> bool {
    let c = cmd.trim();
    // 单独 sudo -i (含 sudo -i\r 等)
    if c == "sudo -i" || c.starts_with("sudo -i ") {
        return true;
    }
    // export REAL_USER=... 后接 export PS1=... + clear (ssh-ops marker.sh 注入的标志)
    if c.starts_with("export REAL_USER=") && c.contains("PS1") {
        return true;
    }
    // 单独 PS1 设置 + clear
    if c.contains("export PS1=") && c.ends_with("clear") {
        return true;
    }
    // ssh-ops marker 切片相关 (SSHOPS_BEGIN/END)
    if c.contains("SSHOPS_BEGIN_") || c.contains("SSHOPS_END_") {
        return true;
    }
    false
}

/// 从 cast 事件流中提取所有"用户输入序列" — 一次输入序列 = 连续的 'i' 事件
/// 直到遇到回车 \r 或 \n 结束。处理 backspace () 删除前一个字符。
///
/// 返回: Vec<(start_elapsed, end_elapsed, command_string)>
pub fn extract_input_groups(events: &[(f64, String)]) -> Vec<(f64, f64, String)> {
    let mut groups: Vec<(f64, f64, String)> = Vec::new();
    let mut elapsed = 0.0_f64;
    let mut buf = String::new();
    let mut buf_start: Option<f64> = None;

    for (delay, line) in events {
        elapsed += *delay;
        let parts: Vec<serde_json::Value> = match serde_json::from_str(line.trim()) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if parts.len() < 3 {
            continue;
        }
        let etype = parts[1].as_str().unwrap_or("");
        if etype != "i" {
            continue;
        }
        let data = parts[2].as_str().unwrap_or("");

        if buf_start.is_none() {
            buf_start = Some(elapsed);
        }

        for ch in data.chars() {
            match ch {
                '\r' | '\n' => {
                    let cmd = buf.trim().to_string();
                    if !cmd.is_empty() {
                        groups.push((buf_start.unwrap_or(elapsed), elapsed, cmd));
                    }
                    buf.clear();
                    buf_start = None;
                }
                '\u{007f}' | '\u{0008}' => {
                    // backspace
                    buf.pop();
                }
                c if c.is_control() => {
                    // 其他控制字符忽略 (Ctrl+C 等)
                }
                c => buf.push(c),
            }
        }
    }

    groups
}

/// 合并 ssh-ops commands.jsonl 的 AI 命令 + cast events 提取的所有 input groups,
/// 去重: 与 AI 命令 cast_offset ± 1.0s 内重复的 input group 跳过。
/// 给所有命令补上 input_start_offset 字段。
pub fn merge_commands_with_inputs(
    mut ai_commands: Vec<CommandRecord>,
    events: &[(f64, String)],
) -> Vec<CommandRecord> {
    let groups = extract_input_groups(events);

    // 第一步: 给 ai 命令找匹配的 input group, 设 input_start_offset
    for cmd in ai_commands.iter_mut() {
        cmd.actor = if cmd.actor.is_empty() { "ai".to_string() } else { cmd.actor.clone() };
        // 找时间最接近 cast_offset 且命令片段匹配的 group
        let mut best: Option<(f64, f64)> = None; // (input_start, dist)
        for (start, end, content) in &groups {
            let dist = (cmd.cast_offset - *end).abs();
            if dist > 60.0 {
                continue;
            }
            // 命令字符串匹配 (含子串关系都行, ai 命令可能是组合)
            let matched = cmd.cmd.contains(content)
                || content.contains(&cmd.cmd)
                || cmd.cmd.split_whitespace().next() == content.split_whitespace().next();
            if matched {
                match best {
                    Some((_, d)) if dist >= d => {}
                    _ => best = Some((*start, dist)),
                }
            }
        }
        if let Some((s, _)) = best {
            cmd.input_start_offset = s;
        } else {
            cmd.input_start_offset = (cmd.cast_offset
                - (cmd.duration_ms as f64) / 1000.0
                - 3.0)
                .max(0.0);
        }
    }

    // 第二步: 找未被任何 ai 命令匹配的 input groups, 创建 human 命令
    let mut human_commands: Vec<CommandRecord> = Vec::new();
    for (start, end, content) in groups {
        // 与任意 ai 命令的 input_start_offset 距离 ≤ 1s 视为已匹配
        let claimed = ai_commands.iter().any(|c| (c.input_start_offset - start).abs() < 1.0);
        if claimed {
            continue;
        }
        // 跳过仅含单字符或纯空白的输入
        if content.trim().len() <= 1 {
            continue;
        }
        // 跳过 ssh-ops 自动注入的初始化命令 (基础设施, 不是真实 human 输入)
        if is_sshops_bootstrap_command(&content) {
            continue;
        }
        human_commands.push(CommandRecord {
            ts: String::new(),
            actor: "human".to_string(),
            host: String::new(),
            cmd: content.clone(),
            exit: 0,
            duration_ms: ((end - start) * 1000.0) as u64,
            cast_offset: end,
            input_start_offset: start,
            dangerous: false,
            blocked: false,
            nonce: format!("human-{}", (start * 1000.0) as i64),
        });
    }

    let mut merged = ai_commands;
    merged.extend(human_commands);
    merged.sort_by(|a, b| a.cast_offset.partial_cmp(&b.cast_offset).unwrap_or(std::cmp::Ordering::Equal));
    merged
}