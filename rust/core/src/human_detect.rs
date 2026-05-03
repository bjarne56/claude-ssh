//! ai/human 命令检测 (从 cast 字节流提取 input groups, 按 chunk_size 分类)

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HumanCmd {
    pub cmd: String,
    pub cast_offset: f64,
    pub ts_unix: u64,
}

#[derive(Debug, Clone)]
pub struct InputGroup {
    pub start_elapsed: f64,
    pub end_elapsed: f64,
    pub content: String,
    pub max_chunk_size: usize,
    pub chunk_count: usize,
}

impl InputGroup {
    pub fn is_human(&self) -> bool {
        self.max_chunk_size <= 3
    }
}

/// 从 cast 文件 (JSONL) 字节区间提取 input groups
pub fn extract_input_groups(cast_bytes: &[u8]) -> Vec<InputGroup> {
    let mut groups = Vec::new();
    let mut elapsed = 0.0_f64;
    let mut buf = String::new();
    let mut buf_start: Option<f64> = None;
    let mut max_chunk = 0_usize;
    let mut chunk_count = 0_usize;

    for line in cast_bytes.split(|&b| b == b'\n') {
        let line = std::str::from_utf8(line).unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let parts: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let arr = match parts.as_array() {
            Some(a) if a.len() >= 3 => a,
            _ => continue,
        };
        let delay = arr[0].as_f64().unwrap_or(0.0);
        elapsed += delay;
        let etype = arr[1].as_str().unwrap_or("");
        if etype != "i" {
            continue;
        }
        let data = arr[2].as_str().unwrap_or("");
        let visible = data.chars().filter(|c| !c.is_control()).count();
        if visible > max_chunk {
            max_chunk = visible;
        }
        chunk_count += 1;
        if buf_start.is_none() {
            buf_start = Some(elapsed);
        }
        for ch in data.chars() {
            match ch {
                '\r' | '\n' => {
                    let cmd = buf.trim().to_string();
                    if !cmd.is_empty() {
                        groups.push(InputGroup {
                            start_elapsed: buf_start.unwrap_or(elapsed),
                            end_elapsed: elapsed,
                            content: cmd,
                            max_chunk_size: max_chunk,
                            chunk_count,
                        });
                    }
                    buf.clear();
                    buf_start = None;
                    max_chunk = 0;
                    chunk_count = 0;
                }
                '\u{007f}' | '\u{0008}' => {
                    buf.pop();
                }
                c if c.is_control() => {}
                c => buf.push(c),
            }
        }
    }
    groups
}