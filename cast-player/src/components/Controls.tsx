import type { CommandRecord, CastEventMeta } from "../types";
import type { usePlayer } from "../usePlayer";

type PlayerHook = ReturnType<typeof usePlayer>;

function fmtTime(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = Math.floor(seconds % 60);
  return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
}

interface ControlsProps {
  player: PlayerHook;
  commands: CommandRecord[];
  events: CastEventMeta[];
  totalDuration: number;
}

export function Controls({
  player,
  commands,
  totalDuration,
}: ControlsProps) {
  const {
    playState,
    elapsed,
    speed,
    skipIdle,
    togglePlay,
    stop,
    stepForward,
    stepBackward,
    cycleSpeed,
    toggleSkipIdle,
    restart,
    seekTo,
  } = player;

  const progress = totalDuration > 0 ? (elapsed / totalDuration) * 100 : 0;

  const handleProgressClick = (e: React.MouseEvent<HTMLDivElement>) => {
    const rect = e.currentTarget.getBoundingClientRect();
    const pct = (e.clientX - rect.left) / rect.width;
    seekTo(pct * totalDuration);
  };

  // 命令标记点
  const markers = commands
    .filter((c) => c.cast_offset > 0 && c.cast_offset <= totalDuration)
    .map((c) => ({
      pct: (c.cast_offset / totalDuration) * 100,
      dangerous: c.dangerous,
      key: c.nonce || c.ts + c.cmd,
    }));

  return (
    <>
      <div className="progress-wrapper">
        <span className="time" style={{ minWidth: 52 }}>
          {fmtTime(elapsed)}
        </span>
        <div className="progress-bar" onClick={handleProgressClick}>
          <div className="filled" style={{ width: `${progress}%` }} />
          {markers.map((m) => (
            <div
              key={m.key}
              className={`marker ${m.dangerous ? "cmd-danger" : "cmd"}`}
              style={{ left: `${m.pct}%` }}
            />
          ))}
        </div>
        <span className="time" style={{ minWidth: 52 }}>
          {fmtTime(totalDuration)}
        </span>
      </div>
      <div className="controls-bar">
        <div className="group">
          <button
            onClick={() => stepBackward(30)}
            title="快退 30s"
            disabled={playState === "idle"}
          >
            ⏪ 30
          </button>
          <button
            onClick={() => stepBackward(10)}
            title="快退 10s"
            disabled={playState === "idle"}
          >
            ◀◀ 10
          </button>
          <button
            onClick={() => stepBackward(5)}
            title="快退 5s"
            disabled={playState === "idle"}
          >
            ◀ 5
          </button>
        </div>
        <div className="group">
          <button onClick={togglePlay} title={playState === "playing" ? "暂停" : "播放"}>
            {playState === "playing" ? "⏸" : "▶"}
          </button>
          <button onClick={stop} title="停止" disabled={playState === "idle"}>
            ⏹
          </button>
          <button onClick={restart} title="重新开始" disabled={playState === "idle"}>
            ↺
          </button>
        </div>
        <div className="group">
          <button
            onClick={() => stepForward(5)}
            title="快进 5s"
            disabled={playState === "idle"}
          >
            5 ▶
          </button>
          <button
            onClick={() => stepForward(10)}
            title="快进 10s"
            disabled={playState === "idle"}
          >
            10 ▶▶
          </button>
          <button
            onClick={() => stepForward(30)}
            title="快进 30s"
            disabled={playState === "idle"}
          >
            30 ⏩
          </button>
        </div>
        <div className="spacer" />
        <div className="group">
          <button
            onClick={toggleSkipIdle}
            title="自动跳过空闲时间"
            className={skipIdle ? "primary" : ""}
          >
            {skipIdle ? "⚡" : "⏳"}
          </button>
          <button onClick={cycleSpeed} title="切换倍速" className="speed-badge">
            {speed}x
          </button>
        </div>
      </div>
    </>
  );
}