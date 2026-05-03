import { useState, useEffect, useCallback } from "react";
import type { SessionSummary } from "../types";
import {
  getDefaultVideoDir,
  scanSessions,
  searchSessions,
  setAppLocale,
} from "../tauri-api";
import { useTranslation, getLocale, setLocale } from "../i18n";

// 支持的语言列表 (locale code → 显示名)
const LOCALES: { code: string; name: string }[] = [
  { code: "en", name: "English" },
  { code: "zh-CN", name: "简体中文" },
  { code: "zh-TW", name: "繁體中文" },
  { code: "ja", name: "日本語" },
  { code: "ko", name: "한국어" },
  { code: "fr", name: "Français" },
  { code: "de", name: "Deutsch" },
  { code: "es", name: "Español" },
  { code: "it", name: "Italiano" },
  { code: "pt-BR", name: "Português (BR)" },
  { code: "pt", name: "Português" },
  { code: "ru", name: "Русский" },
  { code: "uk", name: "Українська" },
  { code: "pl", name: "Polski" },
  { code: "cs", name: "Čeština" },
  { code: "hu", name: "Magyar" },
  { code: "ro", name: "Română" },
  { code: "nl", name: "Nederlands" },
  { code: "sv", name: "Svenska" },
  { code: "nb", name: "Norsk Bokmål" },
  { code: "da", name: "Dansk" },
  { code: "fi", name: "Suomi" },
  { code: "el", name: "Ελληνικά" },
  { code: "ar", name: "العربية" },
  { code: "he", name: "עברית" },
  { code: "tr", name: "Türkçe" },
  { code: "hi", name: "हिन्दी" },
  { code: "id", name: "Indonesia" },
  { code: "ms", name: "Melayu" },
  { code: "fil", name: "Filipino" },
  { code: "vi", name: "Tiếng Việt" },
  { code: "th", name: "ไทย" },
];

interface SessionSidebarProps {
  onSelect: (session: SessionSummary) => void;
  activeSessionId: string | null;
}

export function SessionSidebar({
  onSelect,
  activeSessionId,
}: SessionSidebarProps) {
  const { t } = useTranslation();
  const [sessions, setSessions] = useState<SessionSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [videoDir, setVideoDir] = useState("");
  const [searchQuery, setSearchQuery] = useState("");
  const [projectFilter, setProjectFilter] = useState("");
  const [currentLocale, setCurrentLocale] = useState(getLocale());

  const handleLocaleChange = (newLocale: string) => {
    setLocale(newLocale);
    localStorage.setItem("cast-player-locale", newLocale);
    setCurrentLocale(newLocale);
    // 通知 Rust 重建系统菜单
    setAppLocale(newLocale);
    // 强制整个应用重渲染
    window.dispatchEvent(new Event("locale-changed"));
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
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: 8 }}>
          <h2>{t("sidebar.title")}</h2>
          <select
            value={currentLocale}
            onChange={(e) => handleLocaleChange(e.target.value)}
            title={t("app.language")}
            style={{ fontSize: 11, maxWidth: 140 }}
          >
            {LOCALES.map((l) => (
              <option key={l.code} value={l.code}>
                {l.name}
              </option>
            ))}
          </select>
        </div>
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
            <div className="host">{s.host}</div>
            <div className="meta">
              <span>{s.user}</span>
              <span>{s.command_count} {t("sidebar.commandsCount")}</span>
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