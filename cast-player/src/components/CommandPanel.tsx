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
}

function formatTime(ts: string): string {
  if (!ts) return "";
  try {
    const d = new Date(ts);
    return d.toLocaleTimeString();
  } catch {
    return ts.slice(11, 19) || ts;
  }
}

export function CommandPanel({
  commands,
  elapsed,
  onSeek,
  totalDuration: _totalDuration,
  commandCount,
  dangerousCount,
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
          return (
            <div
              key={cmd.nonce || `${cmd.ts}-${i}`}
              className={`command-item ${isDangerous ? "dangerous" : ""} ${isActive ? "active" : ""}`}
              onClick={() => onSeek(cmd.cast_offset)}
              title={`${t("commands.castOffset")}: ${cmd.cast_offset.toFixed(1)}s | ${t("commands.duration")}: ${cmd.duration_ms}ms | ${t("commands.exitCode")}: ${cmd.exit}`}
            >
              <div className="cmd-text">
                {filter ? highlightText(cmd.cmd, filter) : cmd.cmd}
              </div>
              <div className="cmd-meta">
                <span>{formatTime(cmd.ts)}</span>
                <span>{cmd.actor}</span>
                <span>{cmd.cast_offset.toFixed(1)}s</span>
                <span>exit:{cmd.exit}</span>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}