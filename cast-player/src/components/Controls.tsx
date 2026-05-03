import type { CommandRecord, CastEventMeta } from "../types";
import type { usePlayer } from "../usePlayer";
import { computeIdleSegments } from "../usePlayer";
import { useTranslation } from "../i18n";
import { useMemo } from "react";

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
  events,
  totalDuration,
}: ControlsProps) {
  const { t } = useTranslation();
  const {
    playState,
    elapsed,
    speed,
    skipIdle,
    togglePlay,
    stop,
    stepForward,
    stepBackward,
    toggleSkipIdle,
    restart,
    seekTo,
    jumpToExact,
    SPEEDS,
  } = player;

  const progress = totalDuration > 0 ? (elapsed / totalDuration) * 100 : 0;

  const handleProgressClick = (e: React.MouseEvent<HTMLDivElement>) => {
    const rect = e.currentTarget.getBoundingClientRect();
    const pct = (e.clientX - rect.left) / rect.width;
    seekTo(pct * totalDuration);
  };

  const markers = commands
    .filter((c) => c.cast_offset > 0 && c.cast_offset <= totalDuration)
    .map((c) => ({
      pct: (c.cast_offset / totalDuration) * 100,
      dangerous: c.dangerous,
      key: c.nonce || c.ts + c.cmd,
    }));

  const idleSegments = useMemo(() => {
    if (totalDuration <= 0) return [];
    const segs = computeIdleSegments(events);
    return segs.map((s, i) => ({
      key: `idle-${i}`,
      left: (s.start / totalDuration) * 100,
      width: ((s.end - s.start) / totalDuration) * 100,
    }));
  }, [events, totalDuration]);

  const idle = playState === "idle";

  return (
    <div className="controls-bar-wrapper">
      {/* 播放控制按钮 — 进度条上方 */}
      <div className="controls-bar">
        <div className="group">
          <button onClick={togglePlay} title={playState === "playing" ? t("controls.pause") : t("controls.play")}>
            {playState === "playing" ? "⏸" : "▶"}
          </button>
          <button onClick={stop} title={t("controls.stop")} disabled={idle}>
            ⏹
          </button>
          <button onClick={restart} title={t("controls.restart")} disabled={idle}>
            ↺
          </button>
        </div>

        <div className="group">
          <button onClick={() => stepBackward(30)} title={t("controls.stepBack30")} disabled={idle}>⏪ 30</button>
          <button onClick={() => stepBackward(10)} title={t("controls.stepBack10")} disabled={idle}>◀◀ 10</button>
          <button onClick={() => stepBackward(5)} title={t("controls.stepBack5")} disabled={idle}>◀ 5</button>
        </div>
        <div className="group">
          <button onClick={() => stepForward(5)} title={t("controls.stepForward5")} disabled={idle}>5 ▶</button>
          <button onClick={() => stepForward(10)} title={t("controls.stepForward10")} disabled={idle}>10 ▶▶</button>
          <button onClick={() => stepForward(30)} title={t("controls.stepForward30")} disabled={idle}>30 ⏩</button>
        </div>

        <div className="spacer" />

        {/* 倍速切换 */}
        <div className="group">
          <span style={{ fontSize: 11, color: "var(--text-muted)", marginRight: 4 }}>{t("controls.speedLabel")}</span>
          {SPEEDS.map((s) => (
            <button
              key={s}
              onClick={() => player.changeSpeed(s)}
              className={s === speed ? "primary" : ""}
              style={{ minWidth: 32, justifyContent: "center" }}
            >
              {s}x
            </button>
          ))}
        </div>

        <div className="group">
          <button onClick={toggleSkipIdle} title={t("controls.skipIdle")} className={skipIdle ? "primary" : ""}>
            {skipIdle ? t("controls.skipIdleOn") : t("controls.skipIdleOff")}
          </button>
        </div>

        {/* 跳到指定时间戳 */}
        <div className="group">
          <input
            type="text"
            placeholder={t("controls.jumpPlaceholder")}
            style={{ width: 72, fontSize: 11, textAlign: "center" }}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                const v = parseFloat((e.target as HTMLInputElement).value);
                if (!isNaN(v)) jumpToExact(v);
                (e.target as HTMLInputElement).value = "";
              }
            }}
          />
        </div>
      </div>

      {/* 进度条 — 在按钮下方 */}
      <div className="progress-wrapper">
        <span className="time">{fmtTime(elapsed)}</span>
        <div className="progress-bar" onClick={handleProgressClick} title={t("controls.progressClickHint")}>
          {/* 空闲段背景标记 (放底层, 在 filled 之下) */}
          {idleSegments.map((s) => (
            <div
              key={s.key}
              className="idle-segment"
              style={{ left: `${s.left}%`, width: `${s.width}%` }}
              title={t("controls.idleSegmentHint")}
            />
          ))}
          <div className="filled" style={{ width: `${progress}%` }} />
          {markers.map((m) => (
            <div
              key={m.key}
              className={`marker ${m.dangerous ? "cmd-danger" : "cmd"}`}
              style={{ left: `${m.pct}%` }}
              title={m.dangerous ? t("controls.markerDanger") : t("controls.markerCmd")}
            />
          ))}
        </div>
        <span className="time">{fmtTime(totalDuration)}</span>
      </div>
    </div>
  );
}