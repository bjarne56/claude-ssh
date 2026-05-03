import { useRef, useState, useCallback, useEffect } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebglAddon } from "@xterm/addon-webgl";
import "@xterm/xterm/css/xterm.css";
import type { LoadResult, CastEventMeta, PlayState } from "./types";

export const SPEEDS = [0.5, 1, 1.5, 2, 4, 8];
export const IDLE_THRESHOLD = 2.0;

/** 计算空闲段: 相邻事件间 delay > IDLE_THRESHOLD 的时间区间 */
export function computeIdleSegments(
  events: { elapsed: number }[]
): { start: number; end: number }[] {
  const segs: { start: number; end: number }[] = [];
  for (let i = 1; i < events.length; i++) {
    const delay = events[i].elapsed - events[i - 1].elapsed;
    if (delay > IDLE_THRESHOLD) {
      segs.push({ start: events[i - 1].elapsed, end: events[i].elapsed });
    }
  }
  return segs;
}

export function usePlayer() {
  // ---- refs (不触发重渲染) ----
  const termRef = useRef<Terminal | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const timerRef = useRef<number | null>(null);
  const eventIdxRef = useRef(0);
  const eventsRef = useRef<[number, string][]>([]);
  const indexRef = useRef<CastEventMeta[]>([]);
  const totalRef = useRef(0);
  const speedRef = useRef(1);
  const skipIdleRef = useRef(true);
  const playStateRef = useRef<PlayState>("idle");

  // ---- state (触发 UI 更新) ----
  const [playState, setPlayState] = useState<PlayState>("idle");
  const [elapsed, setElapsed] = useState(0);
  const [totalDuration, setTotalDuration] = useState(0);
  const [speed, setSpeed] = useState(1);
  const [skipIdle, setSkipIdle] = useState(true);

  const setPlayState2 = useCallback((s: PlayState) => {
    playStateRef.current = s;
    setPlayState(s);
  }, []);

  // ---- 终端初始化 ----
  const initTerminal = useCallback((container: HTMLDivElement) => {
    if (termRef.current) termRef.current.dispose();

    const term = new Terminal({
      fontFamily: "'SF Mono', 'JetBrains Mono', 'Fira Code', monospace",
      fontSize: 13,
      theme: { background: "#000000", foreground: "#cdd6f4" },
      cursorBlink: false,
      disableStdin: true,
      allowProposedApi: true,
      scrollback: 10000,
    });

    const fitAddon = new FitAddon();
    term.loadAddon(fitAddon);
    try { term.loadAddon(new WebglAddon()); } catch {}

    term.open(container);
    fitAddon.fit();

    termRef.current = term;
    fitAddonRef.current = fitAddon;

    const observer = new ResizeObserver(() => { try { fitAddon.fit(); } catch {} });
    observer.observe(container);

    return () => {
      observer.disconnect();
      term.dispose();
      termRef.current = null;
    };
  }, []);

  // ---- 核心: tick (只依赖 ref,零重渲染) ----
  const tickRef = useRef<() => void>(() => {});

  tickRef.current = () => {
    const events = eventsRef.current;
    const idx = eventIdxRef.current;
    if (idx >= events.length) {
      if (timerRef.current !== null) {
        clearTimeout(timerRef.current);
        timerRef.current = null;
      }
      setPlayState2("stopped");
      return;
    }

    // 写当前事件 (只渲染 "o" 输出, 跳过 "i" 输入和 "x" 退出)
    const [, line] = events[idx];
    const parts = JSON.parse(line) as [number, string, string];
    if (parts.length >= 3 && parts[1] === "o") {
      termRef.current?.write(parts[2].replace(/\0/g, ""));
    }

    // elapsed = 当前事件的累计时间
    const meta = indexRef.current;
    setElapsed(meta[idx]?.elapsed ?? totalRef.current);

    // 推进
    eventIdxRef.current = idx + 1;

    // 用 *下一帧* 的 delay 作为等待时间 (cast v3 语义)
    if (idx + 1 >= events.length) {
      // 已是最后一帧
      timerRef.current = null;
      setPlayState2("stopped");
      return;
    }
    const [nextDelay] = events[idx + 1];
    let delay = nextDelay;
    if (skipIdleRef.current && delay > IDLE_THRESHOLD) {
      delay = 0.1;
    }
    const realDelay = (delay * 1000) / speedRef.current;

    timerRef.current = window.setTimeout(() => tickRef.current(), Math.max(realDelay, 1));
  };

  const startTimer = useCallback(() => {
    if (timerRef.current !== null) return;
    // 异步调度第一次 tick, 让调用者的 React state 先生效再播放
    timerRef.current = window.setTimeout(() => tickRef.current(), 0);
  }, []);

  const stopTimer = useCallback(() => {
    if (timerRef.current !== null) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
  }, []);

  // ---- seekTo (不依赖 playState state, 用 ref) ----
  const seekTo = useCallback((targetElapsed: number) => {
    const wasPlaying = timerRef.current !== null;
    stopTimer();

    const term = termRef.current;
    if (!term) return;

    const meta = indexRef.current;
    if (meta.length === 0) return;

    // 二分查找
    let lo = 0, hi = meta.length - 1;
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      if (meta[mid].elapsed < targetElapsed) lo = mid + 1;
      else hi = mid;
    }

    eventIdxRef.current = lo;
    setElapsed(targetElapsed);

    // 重建终端
    term.reset();
    for (let i = 0; i < lo; i++) {
      const [, line] = eventsRef.current[i];
      try {
        const parts = JSON.parse(line) as [number, string, string];
        if (parts.length >= 3 && parts[1] === "o") {
          term.write(parts[2].replace(/\0/g, ""));
        }
      } catch {}
    }

    if (wasPlaying) startTimer();
  }, [stopTimer, startTimer]);

  // ---- 控制函数 ----
  const play = useCallback(() => {
    if (playStateRef.current === "playing") return;
    if (eventIdxRef.current >= eventsRef.current.length) {
      seekTo(0);
    }
    setPlayState2("playing");
    startTimer();
  }, [seekTo, startTimer, setPlayState2]);

  const pause = useCallback(() => {
    stopTimer();
    setPlayState2("paused");
  }, [stopTimer, setPlayState2]);

  const stop = useCallback(() => {
    stopTimer();
    seekTo(0);
    setPlayState2("stopped");
  }, [stopTimer, seekTo, setPlayState2]);

  const togglePlay = useCallback(() => {
    if (playStateRef.current === "playing") pause();
    else play();
  }, [play, pause]);

  const stepForward = useCallback((seconds: number) => {
    seekTo(Math.min(elapsed + seconds, totalRef.current));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [elapsed, seekTo]);

  const stepBackward = useCallback((seconds: number) => {
    seekTo(Math.max(elapsed - seconds, 0));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [elapsed, seekTo]);

  const changeSpeed = useCallback((newSpeed: number) => {
    setSpeed(newSpeed);
    speedRef.current = newSpeed;
    if (timerRef.current !== null) {
      stopTimer();
      startTimer();
    }
  }, [stopTimer, startTimer]);

  const toggleSkipIdle = useCallback(() => {
    setSkipIdle((prev) => {
      skipIdleRef.current = !prev;
      return !prev;
    });
  }, []);

  const restart = useCallback(() => {
    seekTo(0);
    setPlayState2("playing");
    startTimer();
  }, [seekTo, startTimer, setPlayState2]);

  const jumpToExact = useCallback((seconds: number) => {
    seekTo(Math.max(0, Math.min(seconds, totalRef.current)));
  }, [seekTo]);

  // ---- session 加载 ----
  // 即使 term 还没初始化也设置 refs, 这样 Player 挂载后初始化的 term 会拿到数据
  const loadSession = useCallback((data: LoadResult) => {
    stopTimer();
    eventsRef.current = data.events;
    indexRef.current = data.index.events;
    totalRef.current = data.index.total_duration;
    eventIdxRef.current = 0;

    // 如果 term 已经存在, reset 它
    termRef.current?.reset();

    setElapsed(0);
    setTotalDuration(data.index.total_duration);
    setPlayState2("idle");
  }, [stopTimer, setPlayState2]);

  // 清理
  useEffect(() => () => stopTimer(), [stopTimer]);

  return {
    initTerminal,
    loadSession,
    playState,
    elapsed,
    totalDuration,
    speed,
    skipIdle,
    play, pause, stop,
    togglePlay, seekTo,
    stepForward, stepBackward,
    changeSpeed, cycleSpeed: () => {
      const idx = SPEEDS.indexOf(speedRef.current);
      changeSpeed(SPEEDS[(idx + 1) % SPEEDS.length]);
    },
    toggleSkipIdle,
    restart,
    jumpToExact,
    SPEEDS,
  };
}