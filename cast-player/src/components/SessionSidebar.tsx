import { useState, useEffect, useCallback } from "react";
import type { SessionSummary } from "../types";
import {
  getDefaultVideoDir,
  scanSessions,
  searchSessions,
  deleteSession,
} from "../tauri-api";
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
      const dir = await getDefaultVideoDir();
      setVideoDir(dir);
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
        <div style={{ fontSize: 11, color: "var(--text-muted)" }}>
          {videoDir ? `📁 ${videoDir}` : t("sidebar.noVideoDir")}
        </div>
        <button
          onClick={loadSessions}
          style={{
            fontSize: 11,
            padding: "2px 6px",
            background: "var(--bg-surface)",
          }}
        >
          {t("sidebar.refresh")}
        </button>
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
        {!loading && !error && filtered.length === 0 && (
          <div className="empty-state">
            <p>{t("sidebar.noSessions")} {videoDir || t("sidebar.notConfigured")}</p>
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
              <span>
                {s.started_at.slice(0, 16).replace("T", " ")}
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