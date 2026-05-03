import { useState, useEffect, useCallback } from "react";
import type { SessionSummary } from "../types";
import {
  scanSessions,
  searchSessions,
  deleteSession,
  getVideoDir,
  setVideoDir as saveVideoDir,
} from "../tauri-api";
import { open } from "@tauri-apps/plugin-dialog";

/** UTC unix 秒 → 本机本地时间 "YYYY-MM-DD HH:mm"
 *  Rust 端 cast.timestamp / started_at 都是 UTC,
 *  Date 对象 + toLocaleString("sv-SE") 输出 ISO 风格本机时区 */
function fmtLocal(unixSec: number | null, fallbackIso: string): string {
  let d: Date;
  if (unixSec && unixSec > 0) {
    d = new Date(unixSec * 1000);
  } else {
    d = new Date(fallbackIso.replace(/\.\d?[A-Za-z]+Z$/i, ".000Z"));
  }
  if (isNaN(d.getTime())) return fallbackIso.slice(0, 16).replace("T", " ");
  // sv-SE 输出 "YYYY-MM-DD HH:mm" 格式 (瑞典 ISO 风格), 自动用本机时区
  return d.toLocaleString("sv-SE", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  });
}
import { useTranslation } from "../i18n";

interface SessionSidebarProps {
  onSelect: (session: SessionSummary) => void;
  activeSessionId: string | null;
  onSessionDeleted?: (sessionId: string) => void;
}

export function SessionSidebar({
  onSelect,
  activeSessionId,
  onSessionDeleted,
}: SessionSidebarProps) {
  const { t } = useTranslation();
  const [sessions, setSessions] = useState<SessionSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [videoDir, setVideoDir] = useState("");
  const [searchQuery, setSearchQuery] = useState("");
  const [projectFilter, setProjectFilter] = useState("");

  const handleDelete = async (e: React.MouseEvent, session: SessionSummary) => {
    e.stopPropagation();
    if (!window.confirm(`确定删除会话 ${session.session_id}？\n该操作不可恢复。`)) {
      return;
    }
    try {
      const dir = session.cast_path.replace(/\/stream\.cast$/, "");
      await deleteSession(dir);
      setSessions((prev) => prev.filter((s) => s.session_id !== session.session_id));
      if (onSessionDeleted) onSessionDeleted(session.session_id);
    } catch (err) {
      alert(`删除失败: ${err}`);
    }
  };

  const loadSessions = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const dir = await getVideoDir();
      setVideoDir(dir);
      if (!dir) {
        // 未配置, 显示首屏让用户选目录
        setSessions([]);
        return;
      }
      const all = await scanSessions(dir);
      setSessions(all);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadSessions();
  }, [loadSessions]);

  const handleChangeDir = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: "选择录像目录",
        defaultPath: videoDir || undefined,
      });
      if (!selected || typeof selected !== "string") return;
      await saveVideoDir(selected);
      await loadSessions();
    } catch (err) {
      alert(`设置失败: ${err}`);
    }
  };

  const handleSearch = useCallback(
    async (q: string) => {
      setSearchQuery(q);
      if (!videoDir) return;
      try {
        const results = await searchSessions(videoDir, q);
        setSessions(results);
      } catch {
        // 忽略搜索错误
      }
    },
    [videoDir]
  );

  const projects = [...new Set(sessions.map((s) => s.project))].sort();

  let filtered = sessions;
  if (projectFilter) {
    filtered = filtered.filter((s) => s.project === projectFilter);
  }

  return (
    <div className="sidebar">
      <div className="sidebar-header">
        <h2>{t("sidebar.title")}</h2>
        <input
          type="text"
          placeholder={t("sidebar.searchPlaceholder")}
          value={searchQuery}
          onChange={(e) => handleSearch(e.target.value)}
        />
        {projects.length > 1 && (
          <select
            value={projectFilter}
            onChange={(e) => setProjectFilter(e.target.value)}
          >
            <option value="">{t("sidebar.allProjects")}</option>
            {projects.map((p) => (
              <option key={p} value={p}>
                {p}
              </option>
            ))}
          </select>
        )}
        <div
          style={{
            fontSize: 11,
            color: "var(--text-muted)",
            wordBreak: "break-all",
            cursor: "pointer",
          }}
          onClick={handleChangeDir}
          title="点击切换录像目录"
        >
          {videoDir ? `📁 ${videoDir}` : `📁 ${t("sidebar.noVideoDir")}`}
        </div>
        <div style={{ display: "flex", gap: 4 }}>
          <button
            onClick={loadSessions}
            style={{
              flex: 1,
              fontSize: 11,
              padding: "2px 6px",
              background: "var(--bg-surface)",
            }}
          >
            {t("sidebar.refresh")}
          </button>
          <button
            onClick={handleChangeDir}
            style={{
              fontSize: 11,
              padding: "2px 6px",
              background: "var(--bg-surface)",
            }}
            title="选择录像目录"
          >
            📂
          </button>
        </div>
      </div>
      <div className="sidebar-list">
        {loading && (
          <div className="empty-state">
            <p>{t("app.loading")}</p>
          </div>
        )}
        {error && (
          <div className="empty-state">
            <p style={{ color: "var(--danger)" }}>{t("sidebar.loadFailed")} {error}</p>
            <button onClick={loadSessions} className="primary">
              {t("sidebar.retry")}
            </button>
          </div>
        )}
        {!loading && !error && filtered.length === 0 && !videoDir && (
          <div className="empty-state">
            <p>{t("sidebar.noVideoDir")}</p>
            <button onClick={handleChangeDir} className="primary">
              📂 选择录像目录
            </button>
          </div>
        )}
        {!loading && !error && filtered.length === 0 && videoDir && (
          <div className="empty-state">
            <p>{t("sidebar.noSessions")} {videoDir}</p>
          </div>
        )}
        {filtered.map((s) => (
          <div
            key={s.session_id}
            className={`session-card ${s.session_id === activeSessionId ? "active" : ""}`}
            onClick={() => onSelect(s)}
          >
            <button
              className="session-delete-btn"
              onClick={(e) => handleDelete(e, s)}
              title={`删除 ${s.session_id}`}
              aria-label="delete"
            >
              <svg viewBox="0 0 24 24" width="12" height="12" fill="currentColor">
                <path d="M9 3v1H4v2h1v13a2 2 0 002 2h10a2 2 0 002-2V6h1V4h-5V3H9zm0 5h2v10H9V8zm4 0h2v10h-2V8z" />
              </svg>
            </button>
            <div className="host">{s.host}</div>
            <div className="meta">
              <span>{s.user}</span>
              <span>
                {s.command_count} {t("sidebar.commandsCount")}
                {s.human_command_count > 0 && (
                  <span style={{ color: "var(--success)", marginLeft: 3 }}>
                    ({s.ai_command_count}+{s.human_command_count})
                  </span>
                )}
              </span>
              <span title={`UTC: ${s.started_at}`}>
                {fmtLocal(s.cast_timestamp, s.started_at)}
              </span>
            </div>
            <div className="meta" style={{ fontSize: 10 }}>
              {s.project}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}