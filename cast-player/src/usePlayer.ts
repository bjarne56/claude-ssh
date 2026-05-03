import { useRef, useState, useCallback, useEffect } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebglAddon } from "@xterm/addon-webgl";
import "@xterm/xterm/css/xterm.css";
import type { LoadResult, CastEventMeta, PlayState } from "./types";

const SPEEDS = [0.5, 1, 1.5, 2, 4, 8];
const IDLE_THRESHOLD = 2.0; // 超过 2 秒无事件视为空闲

export function usePlayer() {
  const termRef = useRef<Terminal | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const containerRef = useRef<HTMLDivElement | null>(null);

  const [playState, setPlayState] = useState<PlayState>("idle");
  const [elapsed, setElapsed] = useState(0);
  const [totalDuration, setTotalDuration] = useState(0);
  const [speed, setSpeed] = useState(1);
  const [skipIdle, setSkipIdle] = useState(false);

  // 回放控制 refs(避免重渲染)
  const timerRef = useRef<number | null>(null);
  const eventIdxRef = useRef(0);
  const eventsRef = useRef<[number, string][]>([]);
  const indexRef = useRef<CastEventMeta[]>([]);
  const totalRef = useRef(0);
  const speedRef = useRef(1);
  const skipIdleRef = useRef(false);

  const initTerminal = useCallback((container: HTMLDivElement) => {
    if (termRef.current) {
      termRef.current.dispose();
    }

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

    try {
      const webglAddon = new WebglAddon();
      webglAddon.onContextLoss(() => {
        webglAddon.dispose();
      });
      term.loadAddon(webglAddon);
    } catch {
      // WebGL 不可用时回退到 Canvas 渲染
    }

    term.open(container);
    fitAddon.fit();

    termRef.current = term;
    fitAddonRef.current = fitAddon;
    containerRef.current = container;

    const onResize = () => {
      try {
        fitAddon.fit();
      } catch {}
    };
    const observer = new ResizeObserver(onResize);
    observer.observe(container);

    return () => {
      observer.disconnect();
      term.dispose();
      termRef.current = null;
      fitAddonRef.current = null;
    };
  }, []);

  const loadSession = useCallback(
    (data: LoadResult) => {
      const term = termRef.current;
      if (!term) return;

      // 清屏
      term.reset();
      eventsRef.current = data.events;
      indexRef.current = data.index.events;
      totalRef.current = data.index.total_duration;
      eventIdxRef.current = 0;

      setElapsed(0);
      setTotalDuration(data.index.total_duration);
      setPlayState("idle");
    },
    []
  );

  const tick = useCallback(() => {
    const events = eventsRef.current;
    const idx = eventIdxRef.current;
    if (idx >= events.length) {
      stopTimer();
      setPlayState("stopped");
      return;
    }

    const [rawDelay, line] = events[idx];
    let delay = rawDelay;
    const spd = speedRef.current;

    // 空闲跳过逻辑
    if (skipIdleRef.current && delay > IDLE_THRESHOLD) {
      delay = Math.min(delay, 0.1); // 跳过 > 2 秒的空闲,压缩到 0.1s
    }

    const realDelay = (delay * 1000) / spd;

    // 写入终端
    const parts = JSON.parse(line) as [number, string, string];
    if (parts.length >= 3) {
      const data = parts[2];
      // 简单 ANSI 清理(保留终端的 ANSI 处理,只去掉 null)
      const clean = data.replace(/\0/g, "");
      termRef.current?.write(clean);
    }

    eventIdxRef.current = idx + 1;
    const idxMeta = indexRef.current;
    const newElapsed =
      idx + 1 < idxMeta.length
        ? idxMeta[idx + 1].elapsed
        : totalRef.current;

    setElapsed(newElapsed);

    timerRef.current = window.setTimeout(tick, Math.max(realDelay, 1));
  }, []);

  const startTimer = useCallback(() => {
    if (timerRef.current !== null) return;
    tick();
  }, [tick]);

  const stopTimer = useCallback(() => {
    if (timerRef.current !== null) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
  }, []);

  const play = useCallback(() => {
    if (playState === "playing") return;
    if (eventIdxRef.current >= eventsRef.current.length) {
      // 已播完,从头开始
      seekTo(0);
    }
    setPlayState("playing");
    startTimer();
  }, [playState, startTimer]);

  const pause = useCallback(() => {
    stopTimer();
    setPlayState("paused");
  }, [stopTimer]);

  const stop = useCallback(() => {
    stopTimer();
    seekTo(0);
    setPlayState("stopped");
  }, [stopTimer]);

  const togglePlay = useCallback(() => {
    if (playState === "playing") pause();
    else play();
  }, [playState, play, pause]);

  const seekTo = useCallback(
    (targetElapsed: number) => {
      stopTimer();

      const term = termRef.current;
      if (!term) return;

      const meta = indexRef.current;
      if (meta.length === 0) return;

      // 二分查找
      let lo = 0;
      let hi = meta.length - 1;
      while (lo < hi) {
        const mid = (lo + hi) >> 1;
        if (meta[mid].elapsed < targetElapsed) {
          lo = mid + 1;
        } else {
          hi = mid;
        }
      }

      eventIdxRef.current = lo;
      setElapsed(targetElapsed);

      // 重建终端状态: 从 0 到 target 的所有输出事件
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

      if (playState === "playing") {
        startTimer();
      }
    },
    [playState, stopTimer, startTimer]
  );

  const stepForward = useCallback(
    (seconds: number) => {
      const target = Math.min(elapsed + seconds, totalRef.current);
      if (playState === "playing") {
        stopTimer();
        seekTo(target);
        startTimer();
      } else {
        seekTo(target);
      }
    },
    [elapsed, playState, stopTimer, seekTo, startTimer]
  );

  const stepBackward = useCallback(
    (seconds: number) => {
      const target = Math.max(elapsed - seconds, 0);
      if (playState === "playing") {
        stopTimer();
        seekTo(target);
        startTimer();
      } else {
        seekTo(target);
      }
    },
    [elapsed, playState, stopTimer, seekTo, startTimer]
  );

  const changeSpeed = useCallback(
    (newSpeed: number) => {
      setSpeed(newSpeed);
      speedRef.current = newSpeed;

      // 如果正在播放,重新调度
      if (playState === "playing") {
        stopTimer();
        startTimer();
      }
    },
    [playState, stopTimer, startTimer]
  );

  const cycleSpeed = useCallback(() => {
    const idx = SPEEDS.indexOf(speed);
    const nextIdx = (idx + 1) % SPEEDS.length;
    changeSpeed(SPEEDS[nextIdx]);
  }, [speed, changeSpeed]);

  const toggleSkipIdle = useCallback(() => {
    setSkipIdle((prev) => {
      const next = !prev;
      skipIdleRef.current = next;
      return next;
    });
  }, []);

  const restart = useCallback(() => {
    seekTo(0);
    if (playState !== "playing") {
      setPlayState("playing");
      startTimer();
    } else {
      startTimer();
    }
  }, [playState, seekTo, startTimer]);

  const jumpToExact = useCallback(
    (seconds: number) => {
      const target = Math.max(0, Math.min(seconds, totalRef.current));
      if (playState === "playing") {
        stopTimer();
        seekTo(target);
        startTimer();
      } else {
        seekTo(target);
      }
    },
    [playState, stopTimer, seekTo, startTimer]
  );

  // 清理
  useEffect(() => {
    return () => stopTimer();
  }, [stopTimer]);

  return {
    initTerminal,
    containerRef,
    loadSession,
    playState,
    elapsed,
    totalDuration,
    speed,
    skipIdle,
    play,
    pause,
    stop,
    togglePlay,
    seekTo,
    stepForward,
    stepBackward,
    changeSpeed,
    cycleSpeed,
    toggleSkipIdle,
    restart,
    jumpToExact,
    SPEEDS,
  };
}