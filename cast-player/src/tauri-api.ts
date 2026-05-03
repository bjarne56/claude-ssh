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

export async function exportCommandsCsv(
  sessionDir: string,
  outputPath: string
): Promise<string> {
  return invoke("export_commands_csv", { sessionDir, outputPath });
}

export async function exportCommandsJson(
  sessionDir: string,
  outputPath: string
): Promise<string> {
  return invoke("export_commands_json", { sessionDir, outputPath });
}

export async function copyCastFile(
  sessionDir: string,
  outputPath: string
): Promise<string> {
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