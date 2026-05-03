import { useState } from "react";
import type { CommandRecord } from "../types";
import { DANGEROUS_PATTERNS } from "../types";
import { useTranslation } from "../i18n";

interface CommandPanelProps {
  commands: CommandRecord[];
  elapsed: number;
  onSeek: (castOffset: number) => void;
  totalDuration: number;
  commandCount: number;
  dangerousCount: number;
  sessionStartedAt: string; // ISO 时间, 用于给无 ts 的 human/inject 命令算时间
}

/** 用 session 起始时间 + cast offset 给 human 命令算实际钟表时间 */
function timeFromOffset(startedAt: string, offset: number): string {
  if (!startedAt) return "";
  const cleaned = startedAt.replace(/\.\d?[A-Za-z]+Z$/i, ".000Z");
  const start = new Date(cleaned);
  if (isNaN(start.getTime())) return "";
  const t = new Date(start.getTime() + offset * 1000);
  return t.toLocaleTimeString(undefined, { hour12: false });
}

/** 解析 ts, 兼容 ssh-ops 写入的 strftime 残留 ".3NZ" 等不规范格式 */
function formatTime(ts: string): string {
  if (!ts) return "";
  // 修正 strftime 残留: ".3NZ" / ".%3NZ" → ".000Z"
  const cleaned = ts.replace(/\.\d?[A-Za-z]+Z$/i, ".000Z").replace(/\.%3NZ$/i, ".000Z");
  const d = new Date(cleaned);
  if (!isNaN(d.getTime())) {
    return d.toLocaleTimeString(undefined, { hour12: false });
  }
  // fallback: 直接从 ISO 字符串截取 HH:MM:SS
  const m = ts.match(/T(\d{2}:\d{2}:\d{2})/);
  return m ? m[1] : ts;
}

export function CommandPanel({
  commands,
  elapsed,
  onSeek,
  totalDuration: _totalDuration,
  commandCount,
  dangerousCount,
  sessionStartedAt,
}: CommandPanelProps) {
  const { t } = useTranslation();
  const [filter, setFilter] = useState("");
  const [regexMode, setRegexMode] = useState(false);
  const [showDangerousOnly, setShowDangerousOnly] = useState(false);

  let filtered = commands;

  if (showDangerousOnly) {
    filtered = filtered.filter(
      (c) =>
        c.dangerous ||
        DANGEROUS_PATTERNS.some((p) => c.cmd.toLowerCase().includes(p))
    );
  }

  if (filter.trim()) {
    const q = filter.trim();
    if (regexMode) {
      try {
        const re = new RegExp(q, "i");
        filtered = filtered.filter((c) => re.test(c.cmd));
      } catch {
        filtered = filtered.filter((c) =>
          c.cmd.toLowerCase().includes(q.toLowerCase())
        );
      }
    } else {
      const lower = q.toLowerCase();
      filtered = filtered.filter((c) => c.cmd.toLowerCase().includes(lower));
    }
  }

  let activeIdx = -1;
  for (let i = 0; i < commands.length; i++) {
    if (commands[i].cast_offset <= elapsed) {
      activeIdx = i;
    } else {
      break;
    }
  }

  const highlightText = (text: string, keyword: string) => {
    if (!keyword.trim()) return text;
    try {
      const re = regexMode
        ? new RegExp(keyword, "gi")
        : new RegExp(keyword.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"), "gi");
      const parts = text.split(re);
      const matches = text.match(re) || [];
      const result: (string | React.ReactNode)[] = [parts[0]];
      for (let i = 0; i < matches.length; i++) {
        result.push(
          <span key={`hl-${i}`} className="highlight">
            {matches[i]}
          </span>
        );
        result.push(parts[i + 1]);
      }
      return result;
    } catch {
      return text;
    }
  };

  return (
    <div className="command-panel">
      <div className="command-panel-header">
        <h3>{t("commands.title")}</h3>
        <div className="command-panel-stats">
          <span>{t("commands.totalCount")}: {commandCount}</span>
          <span style={{ color: "var(--danger)" }}>
            {t("commands.dangerCount")}: {dangerousCount}
          </span>
          <span>{t("commands.filteredCount")}: {filtered.length}</span>
        </div>
        <input
          type="text"
          placeholder={regexMode ? t("commands.regexPlaceholder") : t("commands.searchPlaceholder")}
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
        />
        <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
          <label
            style={{
              fontSize: 11,
              display: "flex",
              alignItems: "center",
              gap: 4,
              cursor: "pointer",
              color: regexMode ? "var(--accent)" : "var(--text-muted)",
            }}
          >
            <input
              type="checkbox"
              checked={regexMode}
              onChange={(e) => setRegexMode(e.target.checked)}
            />
            {t("commands.regexLabel")}
          </label>
          <label
            style={{
              fontSize: 11,
              display: "flex",
              alignItems: "center",
              gap: 4,
              cursor: "pointer",
              color: showDangerousOnly ? "var(--danger)" : "var(--text-muted)",
            }}
          >
            <input
              type="checkbox"
              checked={showDangerousOnly}
              onChange={(e) => setShowDangerousOnly(e.target.checked)}
            />
            {t("commands.dangerOnly")}
          </label>
        </div>
      </div>
      <div className="command-list">
        {filtered.length === 0 && (
          <div
            style={{ padding: 20, textAlign: "center", color: "var(--text-muted)" }}
          >
            {t("commands.noMatch")}
          </div>
        )}
        {filtered.map((cmd, i) => {
          const isDangerous =
            cmd.dangerous ||
            DANGEROUS_PATTERNS.some((p) =>
              cmd.cmd.toLowerCase().includes(p)
            );
          const isActive = commands.indexOf(cmd) === activeIdx;
          const isHuman = cmd.actor === "human";
          // 优先用 input_start_offset (键入起点); 缺失时 fallback cast_offset
          const seekTarget = cmd.input_start_offset > 0
            ? cmd.input_start_offset
            : Math.max(0, cmd.cast_offset - 3);
          return (
            <div
              key={cmd.nonce || `${cmd.ts}-${i}`}
              className={`command-item ${isDangerous ? "dangerous" : ""} ${isActive ? "active" : ""} ${isHuman ? "human" : "ai"}`}
              onClick={() => onSeek(seekTarget)}
              title={`${t("commands.castOffset")}: ${cmd.cast_offset.toFixed(1)}s | ${t("commands.duration")}: ${cmd.duration_ms}ms | ${t("commands.exitCode")}: ${cmd.exit}`}
            >
              <div className="cmd-text">
                {isHuman ? (
                  <svg
                    className="actor-icon"
                    viewBox="0 0 24 24"
                    fill="currentColor"
                    style={{ color: "var(--success)" }}
                    aria-label="human"
                  >
                    <path d="M12 12c2.21 0 4-1.79 4-4s-1.79-4-4-4-4 1.79-4 4 1.79 4 4 4zm0 2c-2.67 0-8 1.34-8 4v2h16v-2c0-2.66-5.33-4-8-4z" />
                  </svg>
                ) : (
                  <svg
                    className="actor-icon"
                    viewBox="0 0 24 24"
                    fill="currentColor"
                    style={{ color: "var(--accent)" }}
                    aria-label="ai"
                  >
                    <path d="M12 2a2 2 0 0 1 2 2v1h4a2 2 0 0 1 2 2v3h1a1 1 0 0 1 1 1v3a1 1 0 0 1-1 1h-1v3a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2v-3H3a1 1 0 0 1-1-1v-3a1 1 0 0 1 1-1h1V7a2 2 0 0 1 2-2h4V4a2 2 0 0 1 2-2zm-3 9a1.5 1.5 0 1 0 0 3 1.5 1.5 0 0 0 0-3zm6 0a1.5 1.5 0 1 0 0 3 1.5 1.5 0 0 0 0-3zM9 16h6v1H9z" />
                  </svg>
                )}
                {filter ? highlightText(cmd.cmd, filter) : cmd.cmd}
              </div>
              <div className="cmd-meta">
                <span>
                  {cmd.ts ? formatTime(cmd.ts) : timeFromOffset(sessionStartedAt, cmd.input_start_offset || cmd.cast_offset)}
                </span>
                <span>{cmd.cast_offset.toFixed(1)}s</span>
                {cmd.exit !== 0 && <span>exit:{cmd.exit}</span>}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}