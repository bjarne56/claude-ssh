import { describe, test, expect, beforeEach, vi } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { usePlayer } from '../src/usePlayer';
import type { LoadResult } from '../src/types';

// 用 mock terminal 替代真实 xterm.js (jsdom 不支持 WebGL/canvas)
vi.mock('@xterm/xterm', () => {
  return {
    Terminal: class {
      reset = vi.fn();
      write = vi.fn();
      open = vi.fn();
      dispose = vi.fn();
      loadAddon = vi.fn();
      buffer = { active: { length: 0, getLine: () => null } };
    },
  };
});
vi.mock('@xterm/addon-fit', () => ({ FitAddon: class { fit = vi.fn(); } }));
vi.mock('@xterm/addon-webgl', () => ({ WebglAddon: class {} }));

function makeFixture(): LoadResult {
  // 仿造一个 5 事件的 cast: 每秒一个输出
  const events: [number, string][] = [
    [0.0, '[0.0,"o","init "]'],
    [1.0, '[1.0,"o","one "]'],
    [1.0, '[1.0,"o","two "]'],
    [1.0, '[1.0,"o","three "]'],
    [1.0, '[1.0,"o","four"]'],
  ];
  return {
    meta: {
      session_id: 's1',
      project: 'p',
      host_resolved: 'h',
      host_selector: '@h',
      user: 'u',
      auth_type: 'key',
      started_at: '2026-01-01T00:00:00Z',
      ended_at: null,
      command_count: 0,
      ai_command_count: 0,
      human_command_count: 0,
      dangerous_count: 0,
      blocked_count: 0,
    },
    commands: [],
    events,
    index: {
      header: { version: 3, width: 80, height: 24, term: null },
      total_duration: 4.0,
      events: [
        { elapsed: 0.0, byte_offset: 0, event_type: 'output' },
        { elapsed: 1.0, byte_offset: 20, event_type: 'output' },
        { elapsed: 2.0, byte_offset: 40, event_type: 'output' },
        { elapsed: 3.0, byte_offset: 60, event_type: 'output' },
        { elapsed: 4.0, byte_offset: 80, event_type: 'output' },
      ],
    },
  };
}

describe('usePlayer hook', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  test('初始状态 idle', () => {
    const { result } = renderHook(() => usePlayer());
    expect(result.current.playState).toBe('idle');
    expect(result.current.elapsed).toBe(0);
    expect(result.current.totalDuration).toBe(0);
    expect(result.current.speed).toBe(1);
    expect(result.current.skipIdle).toBe(false);
  });

  test('initTerminal 成功', () => {
    const { result } = renderHook(() => usePlayer());
    const div = document.createElement('div');
    act(() => {
      result.current.initTerminal(div);
    });
    // 不报错 = 成功
  });

  test('loadSession 后状态正确', () => {
    const { result } = renderHook(() => usePlayer());
    const div = document.createElement('div');
    act(() => result.current.initTerminal(div));

    act(() => result.current.loadSession(makeFixture()));

    expect(result.current.totalDuration).toBe(4.0);
    expect(result.current.playState).toBe('idle');
    expect(result.current.elapsed).toBe(0);
  });

  test('play 进入 playing 状态', () => {
    const { result } = renderHook(() => usePlayer());
    const div = document.createElement('div');
    act(() => result.current.initTerminal(div));
    act(() => result.current.loadSession(makeFixture()));

    act(() => result.current.play());
    expect(result.current.playState).toBe('playing');
  });

  test('pause 从 playing 切到 paused', () => {
    const { result } = renderHook(() => usePlayer());
    const div = document.createElement('div');
    act(() => result.current.initTerminal(div));
    act(() => result.current.loadSession(makeFixture()));

    act(() => result.current.play());
    act(() => result.current.pause());
    expect(result.current.playState).toBe('paused');
  });

  test('togglePlay 在 idle/playing 之间切换', () => {
    const { result } = renderHook(() => usePlayer());
    const div = document.createElement('div');
    act(() => result.current.initTerminal(div));
    act(() => result.current.loadSession(makeFixture()));

    expect(result.current.playState).toBe('idle');
    act(() => result.current.togglePlay());
    expect(result.current.playState).toBe('playing');
    act(() => result.current.togglePlay());
    expect(result.current.playState).toBe('paused');
    act(() => result.current.togglePlay());
    expect(result.current.playState).toBe('playing');
  });

  test('seekTo 跳到指定时间', () => {
    const { result } = renderHook(() => usePlayer());
    const div = document.createElement('div');
    act(() => result.current.initTerminal(div));
    act(() => result.current.loadSession(makeFixture()));

    act(() => result.current.seekTo(2.5));
    expect(result.current.elapsed).toBe(2.5);
  });

  test('stepForward / stepBackward', () => {
    const { result } = renderHook(() => usePlayer());
    const div = document.createElement('div');
    act(() => result.current.initTerminal(div));
    act(() => result.current.loadSession(makeFixture()));

    act(() => result.current.seekTo(2.0));
    expect(result.current.elapsed).toBe(2.0);

    act(() => result.current.stepForward(1));
    expect(result.current.elapsed).toBe(3.0);

    act(() => result.current.stepBackward(2));
    expect(result.current.elapsed).toBe(1.0);
  });

  test('stepForward 不超过 totalDuration', () => {
    const { result } = renderHook(() => usePlayer());
    const div = document.createElement('div');
    act(() => result.current.initTerminal(div));
    act(() => result.current.loadSession(makeFixture()));

    act(() => result.current.seekTo(3.5));
    act(() => result.current.stepForward(100));
    expect(result.current.elapsed).toBe(4.0);
  });

  test('stepBackward 不低于 0', () => {
    const { result } = renderHook(() => usePlayer());
    const div = document.createElement('div');
    act(() => result.current.initTerminal(div));
    act(() => result.current.loadSession(makeFixture()));

    act(() => result.current.seekTo(1.0));
    act(() => result.current.stepBackward(100));
    expect(result.current.elapsed).toBe(0);
  });

  test('changeSpeed 改变倍速', () => {
    const { result } = renderHook(() => usePlayer());
    const div = document.createElement('div');
    act(() => result.current.initTerminal(div));
    act(() => result.current.loadSession(makeFixture()));

    act(() => result.current.changeSpeed(2));
    expect(result.current.speed).toBe(2);
    act(() => result.current.changeSpeed(0.5));
    expect(result.current.speed).toBe(0.5);
  });

  test('cycleSpeed 循环切换', () => {
    const { result } = renderHook(() => usePlayer());
    const div = document.createElement('div');
    act(() => result.current.initTerminal(div));
    act(() => result.current.loadSession(makeFixture()));

    expect(result.current.speed).toBe(1);
    act(() => result.current.cycleSpeed()); // 1 → 1.5
    expect(result.current.speed).toBe(1.5);
    act(() => result.current.cycleSpeed()); // 1.5 → 2
    expect(result.current.speed).toBe(2);
  });

  test('toggleSkipIdle', () => {
    const { result } = renderHook(() => usePlayer());

    expect(result.current.skipIdle).toBe(false);
    act(() => result.current.toggleSkipIdle());
    expect(result.current.skipIdle).toBe(true);
    act(() => result.current.toggleSkipIdle());
    expect(result.current.skipIdle).toBe(false);
  });

  test('stop 重置到 0 并 stopped', () => {
    const { result } = renderHook(() => usePlayer());
    const div = document.createElement('div');
    act(() => result.current.initTerminal(div));
    act(() => result.current.loadSession(makeFixture()));

    act(() => result.current.seekTo(2.0));
    act(() => result.current.play());
    act(() => result.current.stop());

    expect(result.current.elapsed).toBe(0);
    expect(result.current.playState).toBe('stopped');
  });

  test('restart 从头播放', () => {
    const { result } = renderHook(() => usePlayer());
    const div = document.createElement('div');
    act(() => result.current.initTerminal(div));
    act(() => result.current.loadSession(makeFixture()));

    act(() => result.current.seekTo(3.0));
    act(() => result.current.restart());

    expect(result.current.elapsed).toBe(0);
    expect(result.current.playState).toBe('playing');
  });

  test('jumpToExact 跳到任意时间', () => {
    const { result } = renderHook(() => usePlayer());
    const div = document.createElement('div');
    act(() => result.current.initTerminal(div));
    act(() => result.current.loadSession(makeFixture()));

    act(() => result.current.jumpToExact(2.5));
    expect(result.current.elapsed).toBe(2.5);
  });

  test('jumpToExact 越界裁剪', () => {
    const { result } = renderHook(() => usePlayer());
    const div = document.createElement('div');
    act(() => result.current.initTerminal(div));
    act(() => result.current.loadSession(makeFixture()));

    act(() => result.current.jumpToExact(-5));
    expect(result.current.elapsed).toBe(0);
    act(() => result.current.jumpToExact(999));
    expect(result.current.elapsed).toBe(4.0);
  });

  test('SPEEDS 包含 6 个档位', () => {
    const { result } = renderHook(() => usePlayer());
    expect(result.current.SPEEDS).toEqual([0.5, 1, 1.5, 2, 4, 8]);
  });

  test('播放推进时 elapsed 按延迟前进', async () => {
    const { result } = renderHook(() => usePlayer());
    const div = document.createElement('div');
    act(() => result.current.initTerminal(div));
    act(() => result.current.loadSession(makeFixture()));

    act(() => result.current.play());
    expect(result.current.playState).toBe('playing');

    // 推进 setTimeout(0) 触发第一次 tick → 写 events[0], setElapsed(1)
    await act(async () => { await vi.advanceTimersByTimeAsync(0); });
    expect(result.current.elapsed).toBe(1.0);

    // 再推 1 秒 → 第二次 tick → setElapsed(2)
    await act(async () => { await vi.advanceTimersByTimeAsync(1000); });
    expect(result.current.elapsed).toBe(2.0);
  });

  test('暂停后再播放继续而不重置', async () => {
    const { result } = renderHook(() => usePlayer());
    const div = document.createElement('div');
    act(() => result.current.initTerminal(div));
    act(() => result.current.loadSession(makeFixture()));

    act(() => result.current.play());
    await act(async () => { await vi.advanceTimersByTimeAsync(0); });
    expect(result.current.elapsed).toBe(1.0);

    act(() => result.current.pause());
    expect(result.current.playState).toBe('paused');
    expect(result.current.elapsed).toBe(1.0);

    act(() => result.current.play());
    expect(result.current.playState).toBe('playing');
    // 暂停后不重置, elapsed 仍是 1.0
    expect(result.current.elapsed).toBe(1.0);
  });

  test('倍速 2x 时延迟减半', async () => {
    const { result } = renderHook(() => usePlayer());
    const div = document.createElement('div');
    act(() => result.current.initTerminal(div));
    act(() => result.current.loadSession(makeFixture()));

    act(() => result.current.changeSpeed(2));
    act(() => result.current.play());
    await act(async () => { await vi.advanceTimersByTimeAsync(0); });
    expect(result.current.elapsed).toBe(1.0);

    // 2x 速度: 原本 1s 间隔, 现在 500ms
    await act(async () => { await vi.advanceTimersByTimeAsync(500); });
    expect(result.current.elapsed).toBe(2.0);
    await act(async () => { await vi.advanceTimersByTimeAsync(500); });
    expect(result.current.elapsed).toBe(3.0);
  });

  test('seekTo 在播放中保持播放状态', async () => {
    const { result } = renderHook(() => usePlayer());
    const div = document.createElement('div');
    act(() => result.current.initTerminal(div));
    act(() => result.current.loadSession(makeFixture()));

    act(() => result.current.play());
    expect(result.current.playState).toBe('playing');

    act(() => result.current.seekTo(2.0));
    // seekTo 不改 playState, 仍 playing
    expect(result.current.playState).toBe('playing');
    expect(result.current.elapsed).toBe(2.0);
  });
});