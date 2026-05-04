import { invoke } from "@tauri-apps/api/core";
import type { SessionSummary, LoadResult, CommandRecord } from "./types";

export async function getDefaultVideoDir(): Promise<string> {
  return invoke("get_default_video_dir");
}

export async function scanSessions(videoDir: string): Promise<SessionSummary[]> {
  return invoke("scan_sessions", { videoDir });
}

export async function loadSession(
  sessionDir: string
): Promise<LoadResult> {
  return invoke("load_session", { sessionDir });
}

export async function loadCommandsOnly(
  commandsPath: string
): Promise<CommandRecord[]> {
  return invoke("load_commands_only", { commandsPath });
}

export async function searchSessions(
  videoDir: string,
  query: string
): Promise<SessionSummary[]> {
  return invoke("search_sessions", { videoDir, query });
}

// 返回值改为数字 (CSV/JSON 命令条数, CAST 文件字节数), 让前端 t() 拼成功消息
export async function exportCommandsCsv(
  sessionDir: string,
  outputPath: string
): Promise<number> {
  return invoke("export_commands_csv", { sessionDir, outputPath });
}

export async function exportCommandsJson(
  sessionDir: string,
  outputPath: string
): Promise<number> {
  return invoke("export_commands_json", { sessionDir, outputPath });
}

export async function copyCastFile(
  sessionDir: string,
  outputPath: string
): Promise<number> {
  return invoke("copy_cast_file", { sessionDir, outputPath });
}

/// 通知 Rust 端切换 locale, 重建系统菜单
export async function setAppLocale(locale: string): Promise<void> {
  try {
    await invoke("set_app_locale", { locale });
  } catch (e) {
    console.error("设置应用菜单 locale 失败:", e);
  }
}

/// 删除 session 整个目录
export async function deleteSession(sessionDir: string): Promise<string> {
  return invoke("delete_session", { sessionDir });
}

/// 验证用户提供的录像目录是否合法
export async function validateVideoDir(path: string): Promise<string> {
  return invoke("validate_video_dir", { path });
}

/// 获取已配置的录像目录 (空字符串表示未配置)
export async function getVideoDir(): Promise<string> {
  return invoke("get_video_dir");
}

/// 设置录像目录 (持久化到 ~/Library/Application Support/.../config.json)
export async function setVideoDir(path: string): Promise<string> {
  return invoke("set_video_dir", { path });
}