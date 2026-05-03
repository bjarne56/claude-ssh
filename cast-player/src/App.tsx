import { useState, useCallback } from "react";
import type { SessionSummary, LoadResult } from "./types";
import { loadSession } from "./tauri-api";
import { usePlayer } from "./usePlayer";
import { Player } from "./components/Player";
import { Controls } from "./components/Controls";
import { SessionSidebar } from "./components/SessionSidebar";
import { CommandPanel } from "./components/CommandPanel";
import { SearchOverlay } from "./components/SearchOverlay";
import { ExportDialog } from "./components/ExportDialog";
import { detectLocale, setLocale, getLocale } from "./i18n";
import "./index.css";

// 启动时自动检测系统 locale
setLocale(detectLocale());
console.log(`Cast Player locale: ${getLocale()}`);

function App() {
  const player = usePlayer();
  const [activeSession, setActiveSession] = useState<SessionSummary | null>(
    null
  );
  const [loadData, setLoadData] = useState<LoadResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [showExport, setShowExport] = useState(false);
  const [showSearch, setShowSearch] = useState(false);

  const handleSelectSession = useCallback(
    async (session: SessionSummary) => {
      setLoading(true);
      try {
        const dir = session.cast_path.replace(/\/stream\.cast$/, "");
        const data = await loadSession(dir);
        setLoadData(data);
        setActiveSession(session);
        player.loadSession(data);
      } catch (err) {
        console.error("加载 session 失败:", err);
      } finally {
        setLoading(false);
      }
    },
    [player]
  );

  const handleSeekToCommand = useCallback(
    (castOffset: number) => {
      player.seekTo(castOffset);
      if (player.playState !== "playing") {
        player.play();
      }
    },
    [player]
  );

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === " " && e.target === document.body) {
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
    <div
      className={`app-layout ${!hasSession ? "no-sidebar" : ""}`}
      onKeyDown={handleKeyDown}
      tabIndex={-1}
    >
      <SessionSidebar
        onSelect={handleSelectSession}
        activeSessionId={activeSession?.session_id ?? null}
      />

      <div className={`main-area ${hasSession ? "split" : ""}`}>
        {!hasSession && (
          <div className="empty-state">
            <h3>Cast Player</h3>
            <p>从左侧选择一个会话开始回放,或拖放 .cast 文件到此窗口</p>
            <p style={{ fontSize: 11, color: "var(--text-muted)" }}>
              快捷键: Space 播放/暂停, Ctrl+F 搜索
            </p>
          </div>
        )}

        {loading && (
          <div className="empty-state">
            <p>加载中...</p>
          </div>
        )}

        {hasSession && (
          <>
            <div style={{ display: "flex", flexDirection: "column", flex: 1, overflow: "hidden", position: "relative" }}>
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
          </>
        )}

        {/* 状态栏 */}
        <div className="status-bar" style={{ position: "absolute", bottom: 0, left: 280, right: 0 }}>
          {hasSession && (
            <>
              <span>
                {activeSession.host} ({activeSession.user})
              </span>
              <span>时长: {loadData.index.total_duration.toFixed(0)}s</span>
              <span style={{ flex: 1 }} />
              <button onClick={() => setShowSearch(true)} title="搜索 (Ctrl+F)">
                🔍 搜索
              </button>
              <button onClick={() => setShowExport(true)} title="导出">
                📤 导出
              </button>
            </>
          )}
          {!hasSession && (
            <span style={{ flex: 1, textAlign: "center" }}>准备就绪 — 选择一个会话</span>
          )}
        </div>
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