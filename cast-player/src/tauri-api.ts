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