// Tauri 桥接类型 — 与 Rust 端对应

export interface SessionSummary {
  session_id: string;
  project: string;
  host: string;
  user: string;
  started_at: string;
  command_count: number;
  cast_path: string;
  meta_path: string;
  commands_path: string;
}

export interface SessionMeta {
  session_id: string;
  project: string;
  host_resolved: string;
  host_selector: string;
  user: string;
  auth_type: string;
  started_at: string;
  ended_at: string | null;
  command_count: number;
  ai_command_count: number;
  human_command_count: number;
  dangerous_count: number;
  blocked_count: number;
}

export interface CommandRecord {
  ts: string;
  actor: string;
  host: string;
  cmd: string;
  exit: number;
  duration_ms: number;
  cast_offset: number;
  dangerous: boolean;
  blocked: boolean;
  nonce: string;
}

export interface CastHeader {
  version: number;
  width: number | null;
  height: number | null;
  term: {
    cols: number;
    rows: number;
    type: string;
    version: string;
  } | null;
}

export interface CastEventMeta {
  elapsed: number;
  byte_offset: number;
  event_type: "output" | "input" | "exit";
}

export interface CastIndex {
  header: CastHeader;
  events: CastEventMeta[];
  total_duration: number;
}

export interface LoadResult {
  meta: SessionMeta;
  commands: CommandRecord[];
  events: [number, string][];
  index: CastIndex;
}

export type PlayState = "idle" | "playing" | "paused" | "stopped";

export const DANGEROUS_PATTERNS = [
  "rm -rf",
  "shutdown",
  "reboot",
  "mkfs.",
  "dd ",
  "chmod -R 777",
  "> /dev/sd",
  "wipefs",
  "fdisk",
  ":(){",
];