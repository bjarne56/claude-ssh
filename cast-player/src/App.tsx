import { useState, useCallback, useEffect } from "react";
import type { SessionSummary, LoadResult } from "./types";
import { loadSession } from "./tauri-api";
import { usePlayer } from "./usePlayer";
import { Player } from "./components/Player";
import { Controls } from "./components/Controls";
import { SessionSidebar } from "./components/SessionSidebar";
import { CommandPanel } from "./components/CommandPanel";
import { SearchOverlay } from "./components/SearchOverlay";
import { ExportDialog } from "./components/ExportDialog";
import { detectLocale, setLocale, getLocale, useTranslation } from "./i18n";
import { setAppLocale } from "./tauri-api";
import { listen } from "@tauri-apps/api/event";
import "./index.css";

// 启动时优先用户保存的偏好, 否则自动检测
const savedLocale = typeof localStorage !== "undefined"
  ? localStorage.getItem("cast-player-locale")
  : null;
setLocale(savedLocale || detectLocale());
console.log(`Cast Player locale: ${getLocale()}`);
// 同步到 Rust 端构建系统菜单 (异步, 不阻塞渲染)
setAppLocale(getLocale());

function App() {
  const { t } = useTranslation();
  const player = usePlayer();
  const [activeSession, setActiveSession] = useState<SessionSummary | null>(null);
  const [loadData, setLoadData] = useState<LoadResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [showExport, setShowExport] = useState(false);
  const [showSearch, setShowSearch] = useState(false);
  const [, setLangTick] = useState(0);

  // 监听全局语言变化事件, 强制整个 App 重渲染
  useEffect(() => {
    const handler = () => setLangTick((n) => n + 1);
    window.addEventListener("locale-changed", handler);
    return () => window.removeEventListener("locale-changed", handler);
  }, []);

  // 监听 Rust 端 (顶部 Language 菜单) 触发的语言切换
  useEffect(() => {
    const unlisten = listen<string>("locale-changed-from-menu", (event) => {
      const newLocale = event.payload;
      setLocale(newLocale);
      localStorage.setItem("cast-player-locale", newLocale);
      setLangTick((n) => n + 1);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const handleSelectSession = useCallback(
    async (session: SessionSummary) => {
      setLoading(true);
      try {
        const dir = session.cast_path.replace(/\/stream\.cast$/, "");
        const data = await loadSession(dir);
        // 注意: 不在这里调用 player.loadSession,
        // 因为此时 Player 组件还没挂载, termRef 是 null。
        // 改由下面的 useEffect 在 loadData 变化时调用 (此时 Player 已挂载)。
        setLoadData(data);
        setActiveSession(session);
        console.log(
          `Loaded session: ${data.events.length} events, ${data.index.total_duration.toFixed(1)}s, ${data.commands.length} cmds`
        );
      } catch (err) {
        console.error("加载 session 失败:", err);
      } finally {
        setLoading(false);
      }
    },
    []
  );

  // 关键: 子组件 (Player) 的 useEffect 先于父组件的 useEffect 执行,
  // 所以这里 player.loadSession 调用时 termRef 一定已经设置好了。
  useEffect(() => {
    if (loadData) {
      player.loadSession(loadData);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [loadData]);

  // 点击命令直接跳到 input_start_offset (Rust 端已算好 = 该命令第一个键入字符的 elapsed)
  const handleSeekToCommand = useCallback(
    (inputStartOffset: number) => {
      const target = Math.max(0, inputStartOffset);
      player.seekTo(target);
      if (player.playState !== "playing") player.play();
    },
    [player]
  );

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === " " && (e.target as HTMLElement).tagName !== "INPUT") {
        e.preventDefault();
        player.togglePlay();
      }
      if ((e.metaKey || e.ctrlKey) && e.key === "f") {
        e.preventDefault();
        setShowSearch((prev) => !prev);
      }
    },
    [player]
  );

  const hasSession = loadData !== null && activeSession !== null;
  const sessionDir = activeSession
    ? activeSession.cast_path.replace(/\/stream\.cast$/, "")
    : "";

  return (
    <div className="app-root" onKeyDown={handleKeyDown} tabIndex={-1}>
      <SessionSidebar
        onSelect={handleSelectSession}
        activeSessionId={activeSession?.session_id ?? null}
      />

      <div className="main-column">
        {!hasSession && !loading && (
          <div className="empty-state">
            <h3>{t("app.title")}</h3>
            <p>{t("app.emptyHint")}</p>
            <p style={{ fontSize: 11, color: "var(--text-muted)" }}>
              {t("app.shortcutHint")}
            </p>
          </div>
        )}

        {loading && (
          <div className="empty-state">
            <p>{t("app.loading")}</p>
          </div>
        )}

        {hasSession && (
          <>
            <div className="player-and-cmds">
              <div className="player-column">
                <Player player={player} />
                <Controls
                  player={player}
                  commands={loadData.commands}
                  events={loadData.index.events}
                  totalDuration={loadData.index.total_duration}
                />
                <SearchOverlay
                  termRef={
                    player as unknown as React.MutableRefObject<import("@xterm/xterm").Terminal | null>
                  }
                  visible={showSearch}
                  onClose={() => setShowSearch(false)}
                />
              </div>
              <CommandPanel
                commands={loadData.commands}
                elapsed={player.elapsed}
                onSeek={handleSeekToCommand}
                totalDuration={loadData.index.total_duration}
                commandCount={loadData.meta.command_count}
                dangerousCount={loadData.meta.dangerous_count}
              />
            </div>
            <div className="status-bar">
              <span>
                {activeSession.host} ({activeSession.user})
              </span>
              <span>{t("status.duration")}: {loadData.index.total_duration.toFixed(0)}s</span>
              <span>{t("status.events")}: {loadData.events.length}</span>
              <span>{t("status.state")}: {player.playState}</span>
              <span style={{ flex: 1 }} />
              <button onClick={() => setShowSearch(true)} title={t("search.open")}>
                🔍 {t("search.open").replace(/\s*\(.*\)/, "")}
              </button>
              <button onClick={() => setShowExport(true)} title={t("export.open")}>
                📤 {t("export.open")}
              </button>
            </div>
          </>
        )}
      </div>

      {showExport && sessionDir && (
        <ExportDialog
          sessionDir={sessionDir}
          sessionId={activeSession?.session_id ?? ""}
          onClose={() => setShowExport(false)}
        />
      )}
    </div>
  );
}

export default App;