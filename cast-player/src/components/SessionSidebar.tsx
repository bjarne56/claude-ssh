import { useState, useEffect, useCallback } from "react";
import type { SessionSummary } from "../types";
import {
  getDefaultVideoDir,
  scanSessions,
  searchSessions,
} from "../tauri-api";

interface SessionSidebarProps {
  onSelect: (session: SessionSummary) => void;
  activeSessionId: string | null;
}

export function SessionSidebar({
  onSelect,
  activeSessionId,
}: SessionSidebarProps) {
  const [sessions, setSessions] = useState<SessionSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [videoDir, setVideoDir] = useState("");
  const [searchQuery, setSearchQuery] = useState("");
  const [projectFilter, setProjectFilter] = useState("");

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
        <h2>录像库</h2>
        <input
          type="text"
          placeholder="搜索主机/用户/项目..."
          value={searchQuery}
          onChange={(e) => handleSearch(e.target.value)}
        />
        {projects.length > 1 && (
          <select
            value={projectFilter}
            onChange={(e) => setProjectFilter(e.target.value)}
          >
            <option value="">全部项目</option>
            {projects.map((p) => (
              <option key={p} value={p}>
                {p}
              </option>
            ))}
          </select>
        )}
        <div style={{ fontSize: 11, color: "var(--text-muted)" }}>
          {videoDir ? `📁 ${videoDir}` : "未配置录像目录"}
        </div>
        <button
          onClick={loadSessions}
          style={{
            fontSize: 11,
            padding: "2px 6px",
            background: "var(--bg-surface)",
          }}
        >
          刷新
        </button>
      </div>
      <div className="sidebar-list">
        {loading && (
          <div className="empty-state">
            <p>加载中...</p>
          </div>
        )}
        {error && (
          <div className="empty-state">
            <p style={{ color: "var(--danger)" }}>加载失败: {error}</p>
            <button onClick={loadSessions} className="primary">
              重试
            </button>
          </div>
        )}
        {!loading && !error && filtered.length === 0 && (
          <div className="empty-state">
            <p>暂无录像。录像目录: {videoDir || "未配置"}</p>
          </div>
        )}
        {filtered.map((s) => (
          <div
            key={s.session_id}
            className={`session-card ${s.session_id === activeSessionId ? "active" : ""}`}
            onClick={() => onSelect(s)}
          >
            <div className="host">{s.host}</div>
            <div className="meta">
              <span>{s.user}</span>
              <span>{s.command_count} 命令</span>
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