pub mod cast_index;
pub mod menu_i18n;

use cast_index::*;
use menu_i18n::*;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::menu::{AboutMetadataBuilder, CheckMenuItemBuilder, MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder};
use tauri::{AppHandle, Emitter, Manager};
use walkdir::WalkDir;

// 全局 locale (前端切换时通过 set_app_locale 命令更新)
lazy_static::lazy_static! {
    static ref CURRENT_LOCALE: Mutex<String> = Mutex::new(detect_system_locale());
}

// ---- Tauri Commands ----

#[derive(Debug, Serialize)]
pub struct SessionSummary {
    pub session_id: String,
    pub project: String,
    pub host: String,
    pub user: String,
    pub started_at: String,
    /// AI 命令数 (从 meta.json 读)
    pub ai_command_count: u64,
    /// 手动键入命令数 (从 cast 提取)
    pub human_command_count: u64,
    /// 总命令数 (ai + human, 用于侧栏显示)
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

                // 快速扫描 cast 提取手动命令数 (1132 events ≈ 10ms)
                let human_count = match CastIndex::read_all_events(&cast_path) {
                    Ok(events) => {
                        let groups = extract_input_groups(&events);
                        let ai_cmds = load_commands(&commands_path).unwrap_or_default();
                        let merged = merge_commands_with_inputs(ai_cmds, &events);
                        let _ = groups;
                        merged.iter().filter(|c| c.actor == "human").count() as u64
                    }
                    Err(_) => 0,
                };

                sessions.push(SessionSummary {
                    session_id: meta.session_id.clone(),
                    project,
                    host: meta.host_resolved.clone(),
                    user: meta.user.clone(),
                    started_at: meta.started_at.clone(),
                    ai_command_count: meta.command_count,
                    human_command_count: human_count,
                    command_count: meta.command_count + human_count,
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
    let ai_commands = load_commands(&commands_path).unwrap_or_default();

    let index = CastIndex::build(&cast_path)?;
    let events = CastIndex::read_all_events(&cast_path)?;

    // 合并: ai 命令 + 从 cast 提取的 human 命令
    let commands = merge_commands_with_inputs(ai_commands, &events);

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

/// 删除 session 整个目录 (含 stream.cast / meta.json / commands.jsonl / annotations.jsonl)
#[tauri::command]
fn delete_session(session_dir: String) -> Result<String, String> {
    let dir = PathBuf::from(&session_dir);
    if !dir.exists() {
        return Err(format!("目录不存在: {session_dir}"));
    }
    // 安全检查: 必须是 session 目录 (含 stream.cast 或 meta.json)
    if !dir.join("stream.cast").exists() && !dir.join("meta.json").exists() {
        return Err(format!("非 session 目录, 拒绝删除: {session_dir}"));
    }
    std::fs::remove_dir_all(&dir).map_err(|e| format!("删除失败: {e}"))?;
    Ok(format!("已删除 {}", session_dir))
}

/// 配置文件路径: ~/Library/Application Support/com.ssh-ops.cast-player/config.json
fn config_file_path(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map(|p| p.join("config.json"))
        .map_err(|e| format!("获取配置目录失败: {e}"))
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct AppConfig {
    pub video_dir: Option<String>,
}

fn load_config(app: &AppHandle) -> AppConfig {
    let path = match config_file_path(app) {
        Ok(p) => p,
        Err(_) => return AppConfig::default(),
    };
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_config(app: &AppHandle, cfg: &AppConfig) -> Result<(), String> {
    let path = config_file_path(app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建配置目录失败: {e}"))?;
    }
    let json = serde_json::to_string_pretty(cfg).map_err(|e| format!("序列化失败: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("写入配置失败: {e}"))?;
    Ok(())
}

/// 返回当前配置的录像目录, 没有配置则返回空字符串(前端弹首屏配置)
#[tauri::command]
fn get_video_dir(app: AppHandle) -> String {
    // 优先环境变量 (开发/CI 用)
    if let Ok(d) = std::env::var("SSHOPS_VIDEO_DIR") {
        if PathBuf::from(&d).exists() {
            return d;
        }
    }
    let cfg = load_config(&app);
    cfg.video_dir.unwrap_or_default()
}

/// 设置录像目录到配置文件
#[tauri::command]
fn set_video_dir(app: AppHandle, path: String) -> Result<String, String> {
    let p = PathBuf::from(&path);
    if !p.exists() {
        return Err(format!("目录不存在: {path}"));
    }
    if !p.is_dir() {
        return Err(format!("不是目录: {path}"));
    }
    let mut cfg = load_config(&app);
    cfg.video_dir = Some(path.clone());
    save_config(&app, &cfg)?;
    Ok(path)
}

/// 兼容老前端: 等同 get_video_dir 但路径为空时返回 Err
#[tauri::command]
fn get_default_video_dir(app: AppHandle) -> Result<String, String> {
    let dir = get_video_dir(app);
    if dir.is_empty() {
        Err("未配置录像目录, 请在 UI 设置".to_string())
    } else {
        Ok(dir)
    }
}

/// 验证用户提供的目录是否存在
#[tauri::command]
fn validate_video_dir(path: String) -> Result<String, String> {
    let p = PathBuf::from(&path);
    if !p.exists() {
        return Err(format!("目录不存在: {path}"));
    }
    if !p.is_dir() {
        return Err(format!("不是目录: {path}"));
    }
    Ok(path)
}

/// 前端切换语言时调用, 重建系统菜单
#[tauri::command]
fn set_app_locale(app: AppHandle, locale: String) -> Result<(), String> {
    *CURRENT_LOCALE.lock().unwrap() = locale.clone();
    let menu = build_localized_menu(&app, &locale).map_err(|e| format!("构建菜单失败: {e}"))?;
    app.set_menu(menu).map_err(|e| format!("设置菜单失败: {e}"))?;
    Ok(())
}

#[tauri::command]
fn get_app_locale() -> String {
    CURRENT_LOCALE.lock().unwrap().clone()
}

/// 根据 locale 构建本地化菜单
fn build_localized_menu(app: &AppHandle, locale: &str) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    let l = labels_for(locale);

    // App menu (macOS 左上角第一个菜单, 名字是 app 名)
    let about_metadata = AboutMetadataBuilder::new()
        .name(Some("Cast Player"))
        .version(Some(env!("CARGO_PKG_VERSION")))
        .build();

    let app_submenu = SubmenuBuilder::new(app, "Cast Player")
        .item(&PredefinedMenuItem::about(app, Some(&l.app.about), Some(about_metadata))?)
        .separator()
        .item(&PredefinedMenuItem::services(app, Some(&l.app.services))?)
        .separator()
        .item(&PredefinedMenuItem::hide(app, Some(&l.app.hide))?)
        .item(&PredefinedMenuItem::hide_others(app, Some(&l.app.hide_others))?)
        .item(&PredefinedMenuItem::show_all(app, Some(&l.app.show_all))?)
        .separator()
        .item(&PredefinedMenuItem::quit(app, Some(&l.app.quit))?)
        .build()?;

    // File menu
    let file_submenu = SubmenuBuilder::with_id(app, "file", &l.file.title)
        .item(&PredefinedMenuItem::close_window(app, Some(&l.file.close_window))?)
        .build()?;

    // Edit menu
    let edit_submenu = SubmenuBuilder::with_id(app, "edit", &l.edit.title)
        .item(&PredefinedMenuItem::undo(app, Some(&l.edit.undo))?)
        .item(&PredefinedMenuItem::redo(app, Some(&l.edit.redo))?)
        .separator()
        .item(&PredefinedMenuItem::cut(app, Some(&l.edit.cut))?)
        .item(&PredefinedMenuItem::copy(app, Some(&l.edit.copy))?)
        .item(&PredefinedMenuItem::paste(app, Some(&l.edit.paste))?)
        .item(&PredefinedMenuItem::select_all(app, Some(&l.edit.select_all))?)
        .build()?;

    // View menu
    let fullscreen_item = MenuItemBuilder::with_id("toggle_fullscreen", &l.view.fullscreen)
        .accelerator("CmdOrCtrl+Ctrl+F")
        .build(app)?;
    let view_submenu = SubmenuBuilder::with_id(app, "view", &l.view.title)
        .item(&fullscreen_item)
        .build()?;

    // Window menu
    let window_submenu = SubmenuBuilder::with_id(app, "window", &l.window.title)
        .item(&PredefinedMenuItem::minimize(app, Some(&l.window.minimize))?)
        .item(&PredefinedMenuItem::maximize(app, Some(&l.window.zoom))?)
        .separator()
        .item(&PredefinedMenuItem::bring_all_to_front(app, Some(&l.window.bring_to_front))?)
        .build()?;

    // Language submenu — id 形如 "lang:zh-CN", 选中态用 CheckMenuItem
    let mut lang_builder = SubmenuBuilder::with_id(app, "language", language_menu_title(locale));
    for (code, name) in supported_locales() {
        let item = CheckMenuItemBuilder::with_id(format!("lang:{}", code), name)
            .checked(code == locale)
            .build(app)?;
        lang_builder = lang_builder.item(&item);
    }
    let language_submenu = lang_builder.build()?;

    // Help menu
    let help_submenu = SubmenuBuilder::with_id(app, "help", &l.help.title).build()?;

    MenuBuilder::new(app)
        .item(&app_submenu)
        .item(&file_submenu)
        .item(&edit_submenu)
        .item(&view_submenu)
        .item(&window_submenu)
        .item(&language_submenu)
        .item(&help_submenu)
        .build()
}

// ---- 入口 ----

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // 启动时按系统 locale 建初始菜单
            let initial_locale = CURRENT_LOCALE.lock().unwrap().clone();
            let menu = build_localized_menu(app.handle(), &initial_locale)?;
            app.set_menu(menu)?;
            Ok(())
        })
        .on_menu_event(|app, event| {
            let id = event.id().0.as_str();
            if id == "toggle_fullscreen" {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.set_fullscreen(!window.is_fullscreen().unwrap_or(false));
                }
                return;
            }
            // 语言切换: id 形如 "lang:zh-CN"
            if let Some(new_locale) = id.strip_prefix("lang:") {
                let locale_str = new_locale.to_string();
                *CURRENT_LOCALE.lock().unwrap() = locale_str.clone();
                if let Ok(menu) = build_localized_menu(app, &locale_str) {
                    let _ = app.set_menu(menu);
                }
                // 通知前端同步切换 (前端会更新 i18n + localStorage)
                let _ = app.emit("locale-changed-from-menu", locale_str);
            }
        })
        .invoke_handler(tauri::generate_handler![
            scan_sessions,
            load_session,
            load_commands_only,
            search_sessions,
            export_commands_csv,
            export_commands_json,
            copy_cast_file,
            delete_session,
            get_default_video_dir,
            get_video_dir,
            set_video_dir,
            validate_video_dir,
            set_app_locale,
            get_app_locale,
        ])
        .run(tauri::generate_context!())
        .expect("启动 Tauri 应用失败");
}

// 桌面平台入口
pub fn main() {
    run();
}