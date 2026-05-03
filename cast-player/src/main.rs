mod cast_index;

use cast_index::*;
use serde::Serialize;
use std::path::PathBuf;
use walkdir::WalkDir;

// ---- Tauri Commands ----

#[derive(Debug, Serialize)]
pub struct SessionSummary {
    pub session_id: String,
    pub project: String,
    pub host: String,
    pub user: String,
    pub started_at: String,
    pub command_count: u64,
    pub cast_path: String,
    pub meta_path: String,
    pub commands_path: String,
}

#[derive(Debug, Serialize)]
pub struct LoadResult {
    pub meta: SessionMeta,
    pub commands: Vec<CommandRecord>,
    pub events: Vec<(f64, String)>,
    pub index: CastIndex,
}

/// 扫描录像根目录,返回所有已完成 session 的摘要列表
#[tauri::command]
fn scan_sessions(video_dir: String) -> Result<Vec<SessionSummary>, String> {
    let root = PathBuf::from(&video_dir);
    if !root.exists() {
        return Err(format!("录像目录不存在: {video_dir}"));
    }

    let mut sessions: Vec<SessionSummary> = Vec::new();

    for entry in WalkDir::new(&root)
        .max_depth(3)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_name() == "meta.json" {
            let meta_path = entry.path();
            let session_dir = meta_path.parent().unwrap();
            let cast_path = session_dir.join("stream.cast");
            let commands_path = session_dir.join("commands.jsonl");

            if !cast_path.exists() {
                continue;
            }

            // 尝试读取 meta
            if let Ok(meta) = load_meta(meta_path) {
                // 推断 project 名(父目录)
                let project = session_dir
                    .parent()
                    .and_then(|p| p.file_name())
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();

                sessions.push(SessionSummary {
                    session_id: meta.session_id.clone(),
                    project,
                    host: meta.host_resolved.clone(),
                    user: meta.user.clone(),
                    started_at: meta.started_at.clone(),
                    command_count: meta.command_count,
                    cast_path: cast_path.to_string_lossy().to_string(),
                    meta_path: meta_path.to_string_lossy().to_string(),
                    commands_path: commands_path.to_string_lossy().to_string(),
                });
            }
        }
    }

    // 按时间倒序
    sessions.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    Ok(sessions)
}

/// 加载一个 session 的完整数据: meta + commands + cast 事件 + 索引
#[tauri::command]
fn load_session(session_dir: String) -> Result<LoadResult, String> {
    let dir = PathBuf::from(&session_dir);
    let meta_path = dir.join("meta.json");
    let commands_path = dir.join("commands.jsonl");
    let cast_path = dir.join("stream.cast");

    let meta = load_meta(&meta_path)?;
    let mut commands = load_commands(&commands_path)?;
    sort_commands(&mut commands);

    let index = CastIndex::build(&cast_path)?;
    let events = CastIndex::read_all_events(&cast_path)?;

    Ok(LoadResult {
        meta,
        commands,
        events,
        index,
    })
}

/// 仅加载命令列表(用于回放中刷新)
#[tauri::command]
fn load_commands_only(commands_path: String) -> Result<Vec<CommandRecord>, String> {
    let mut commands = load_commands(&PathBuf::from(&commands_path))?;
    sort_commands(&mut commands);
    Ok(commands)
}

/// 搜索录像目录: 按关键词匹配 host/命令内容
#[tauri::command]
fn search_sessions(video_dir: String, query: String) -> Result<Vec<SessionSummary>, String> {
    let all = scan_sessions(video_dir)?;
    let q = query.to_lowercase();

    if q.is_empty() {
        return Ok(all);
    }

    let filtered: Vec<SessionSummary> = all
        .into_iter()
        .filter(|s| {
            s.host.to_lowercase().contains(&q)
                || s.user.to_lowercase().contains(&q)
                || s.project.to_lowercase().contains(&q)
                || s.session_id.to_lowercase().contains(&q)
        })
        .collect();

    Ok(filtered)
}

// ---- 导出 ----

#[tauri::command]
fn export_commands_csv(session_dir: String, output_path: String) -> Result<String, String> {
    let dir = PathBuf::from(&session_dir);
    let commands_path = dir.join("commands.jsonl");
    let commands = load_commands(&commands_path)?;

    let mut wtr = csv::Writer::from_path(&output_path)
        .map_err(|e| format!("创建 CSV 文件失败: {e}"))?;

    wtr.write_record(&[
        "时间", "操作者", "主机", "命令", "退出码", "耗时ms", "录像偏移s", "危险", "已拦截",
    ])
    .map_err(|e| format!("写 CSV 头失败: {e}"))?;

    for c in &commands {
        let dangerous = if c.dangerous { "是" } else { "否" };
        let blocked = if c.blocked { "是" } else { "否" };
        wtr.write_record(&[
            &c.ts,
            &c.actor,
            &c.host,
            &c.cmd,
            &c.exit.to_string(),
            &c.duration_ms.to_string(),
            &format!("{:.3}", c.cast_offset),
            dangerous,
            blocked,
        ])
        .map_err(|e| format!("写 CSV 行失败: {e}"))?;
    }

    wtr.flush().map_err(|e| format!("flush CSV 失败: {e}"))?;
    Ok(format!("已导出 {} 条命令到 {}", commands.len(), output_path))
}

#[tauri::command]
fn export_commands_json(session_dir: String, output_path: String) -> Result<String, String> {
    let dir = PathBuf::from(&session_dir);
    let commands_path = dir.join("commands.jsonl");
    let commands = load_commands(&commands_path)?;

    let json = serde_json::to_string_pretty(&commands)
        .map_err(|e| format!("序列化 JSON 失败: {e}"))?;
    std::fs::write(&output_path, json).map_err(|e| format!("写入 JSON 文件失败: {e}"))?;

    Ok(format!(
        "已导出 {} 条命令到 {}",
        commands.len(),
        output_path
    ))
}

#[tauri::command]
fn copy_cast_file(session_dir: String, output_path: String) -> Result<String, String> {
    let dir = PathBuf::from(&session_dir);
    let cast_path = dir.join("stream.cast");
    std::fs::copy(&cast_path, &output_path).map_err(|e| format!("复制文件失败: {e}"))?;
    Ok(format!("已复制录像文件到 {}", output_path))
}

#[tauri::command]
fn get_default_video_dir() -> Result<String, String> {
    // 优先用环境变量,其次默认路径
    if let Ok(d) = std::env::var("SSHOPS_VIDEO_DIR") {
        return Ok(d);
    }
    let home = std::env::var("HOME").unwrap_or_default();
    let default = format!("{home}/Code/ssh-op/vedio");
    if PathBuf::from(&default).exists() {
        Ok(default)
    } else {
        Err("未设置录像目录。请设置 SSHOPS_VIDEO_DIR 环境变量或确保 ~/Code/ssh-op/vedio 存在。".to_string())
    }
}

// ---- 入口 ----

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            scan_sessions,
            load_session,
            load_commands_only,
            search_sessions,
            export_commands_csv,
            export_commands_json,
            copy_cast_file,
            get_default_video_dir,
        ])
        .run(tauri::generate_context!())
        .expect("启动 Tauri 应用失败");
}

// 桌面平台入口
pub fn main() {
    run();
}