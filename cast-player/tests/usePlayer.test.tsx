import { describe, test, expect, beforeEach, vi } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { usePlayer, computeIdleSegments, IDLE_THRESHOLD } from '../src/usePlayer';
import type { LoadResult } from '../src/types';

// 用 mock terminal 替代真实 xterm.js (jsdom 不支持 WebGL/canvas)
// 全局共享的 write 日志, 测试间隔离
const writeLog: string[] = [];

vi.mock('@xterm/xterm', () => {
  return {
    Terminal: class {
      reset = vi.fn(() => {
        writeLog.length = 0;
      });
      write = vi.fn((data: string) => {
        writeLog.push(data);
      });
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
    writeLog.length = 0;
  });

  test('初始状态 idle', () => {
    const { result } = renderHook(() => usePlayer());
    expect(result.current.playState).toBe('idle');
    expect(result.current.elapsed).toBe(0);
    expect(result.current.totalDuration).toBe(0);
    expect(result.current.speed).toBe(1);
    // 默认开启跳过空闲
    expect(result.current.skipIdle).toBe(true);
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

  test('toggleSkipIdle (默认 true, 切到 false 再切回)', () => {
    const { result } = renderHook(() => usePlayer());

    expect(result.current.skipIdle).toBe(true);
    act(() => result.current.toggleSkipIdle());
    expect(result.current.skipIdle).toBe(false);
    act(() => result.current.toggleSkipIdle());
    expect(result.current.skipIdle).toBe(true);
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

    // 关掉 skipIdle 让 1s 间隔生效 (>2s 才视为 idle, 1s 不会被跳过)
    act(() => result.current.play());
    expect(result.current.playState).toBe('playing');

    // 推进 setTimeout(0) 触发第一次 tick → 写 events[0], setElapsed(0)
    await act(async () => { await vi.advanceTimersByTimeAsync(0); });
    expect(result.current.elapsed).toBe(0.0);

    // 再推 1 秒 (events[1].delay=1.0) → tick 写 events[1], setElapsed(1)
    await act(async () => { await vi.advanceTimersByTimeAsync(1000); });
    expect(result.current.elapsed).toBe(1.0);

    // 再 1s → events[2], elapsed=2
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
    expect(result.current.elapsed).toBe(0.0);
    await act(async () => { await vi.advanceTimersByTimeAsync(1000); });
    expect(result.current.elapsed).toBe(1.0);

    act(() => result.current.pause());
    expect(result.current.playState).toBe('paused');
    expect(result.current.elapsed).toBe(1.0);

    act(() => result.current.play());
    expect(result.current.playState).toBe('playing');
    expect(result.current.elapsed).toBe(1.0);
  });

  test('倍速 2x 时延迟减半', async () => {
    const { result } = renderHook(() => usePlayer());
    const div = document.createElement('div');
    act(() => result.current.initTerminal(div));
    act(() => result.current.loadSession(makeFixture()));

    act(() => result.current.changeSpeed(2));
    act(() => result.current.play());
    // 第一次 tick (异步 setTimeout 0): 写 events[0], elapsed=0
    await act(async () => { await vi.advanceTimersByTimeAsync(0); });
    expect(result.current.elapsed).toBe(0.0);

    // 2x: 原本 1s, 现在 500ms 触发 events[1]
    await act(async () => { await vi.advanceTimersByTimeAsync(500); });
    expect(result.current.elapsed).toBe(1.0);
    await act(async () => { await vi.advanceTimersByTimeAsync(500); });
    expect(result.current.elapsed).toBe(2.0);
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

  test('回归: 在 initTerminal 前调 loadSession 也不会丢数据', async () => {
    const { result } = renderHook(() => usePlayer());
    const div = document.createElement('div');

    // 先 loadSession (terminal 还没初始化)
    act(() => result.current.loadSession(makeFixture()));
    // 没有报错也没有崩溃

    // 然后 initTerminal
    act(() => result.current.initTerminal(div));

    // 再次 loadSession 应能正常设置 events
    act(() => result.current.loadSession(makeFixture()));

    expect(result.current.totalDuration).toBe(4.0);

    // 点播放应能跑
    act(() => result.current.play());
    await act(async () => { await vi.advanceTimersByTimeAsync(0); });
    expect(result.current.elapsed).toBe(0.0);
    expect(result.current.playState).toBe('playing');
  });

  test('回归: 只渲染 "o" 输出, 跳过 "i" 输入避免重复', async () => {
    const fixture: LoadResult = {
      ...makeFixture(),
      events: [
        // 模拟用户敲 ls + 终端 echo + 输出 + 用户敲 pwd + echo
        [0.0, '[0.0,"o","[user@host]$ "]'],
        [0.5, '[0.5,"i","ls\\r"]'],
        [0.05, '[0.05,"o","ls\\r\\n"]'],
        [0.1, '[0.1,"o","file1 file2\\r\\n"]'],
        [0.0, '[0.0,"o","[user@host]$ "]'],
        [1.0, '[1.0,"i","pwd\\r"]'],
        [0.05, '[0.05,"o","pwd\\r\\n"]'],
        [0.1, '[0.1,"o","/home/user\\r\\n"]'],
      ],
      index: {
        header: { version: 3, width: 80, height: 24, term: null },
        total_duration: 1.8,
        events: [
          { elapsed: 0.0, byte_offset: 0, event_type: 'output' },
          { elapsed: 0.5, byte_offset: 30, event_type: 'input' },
          { elapsed: 0.55, byte_offset: 60, event_type: 'output' },
          { elapsed: 0.65, byte_offset: 90, event_type: 'output' },
          { elapsed: 0.65, byte_offset: 120, event_type: 'output' },
          { elapsed: 1.65, byte_offset: 150, event_type: 'input' },
          { elapsed: 1.7, byte_offset: 180, event_type: 'output' },
          { elapsed: 1.8, byte_offset: 210, event_type: 'output' },
        ],
      },
    };

    const { result } = renderHook(() => usePlayer());
    const div = document.createElement('div');
    act(() => result.current.initTerminal(div));
    act(() => result.current.loadSession(fixture));

    act(() => result.current.play());
    // 推进到全部播完
    await act(async () => { await vi.advanceTimersByTimeAsync(3000); });

    // 验证: 写入终端的内容不该含 "i" 类型的原始数据
    // 只应该有 6 次 write (6 个 o 事件), 不该有 8 次
    const oEventCount = fixture.events.filter((e) => {
      const parsed = JSON.parse(e[1]);
      return parsed[1] === 'o';
    }).length;

    expect(writeLog.length).toBe(oEventCount);
    expect(oEventCount).toBe(6);

    // 验证内容: 不该有 input 的 "ls\r" 或 "pwd\r" 单独出现
    // (它们只能作为 output 的一部分出现, 含 \r\n)
    const allWritten = writeLog.join('');
    // input 数据是 "ls\r" / "pwd\r" (无 \n), output 是 "ls\r\n" / "pwd\r\n"
    // 所以单独 "ls\r" 不该出现 — 但 "ls\r\n" 可以
    expect(allWritten).not.toMatch(/ls\r(?!\n)/);
    expect(allWritten).not.toMatch(/pwd\r(?!\n)/);
    // 但 echo 的 "ls\r\n" 应该有
    expect(allWritten).toContain('ls\r\n');
    expect(allWritten).toContain('pwd\r\n');
  });
});

describe('computeIdleSegments', () => {
  test('空数组', () => {
    expect(computeIdleSegments([])).toEqual([]);
  });

  test('全部 < IDLE_THRESHOLD', () => {
    const events = [
      { elapsed: 0.0 },
      { elapsed: 0.5 },
      { elapsed: 1.0 },
      { elapsed: 1.8 },
    ];
    expect(computeIdleSegments(events)).toEqual([]);
  });

  test('单个 idle 段', () => {
    const events = [
      { elapsed: 0.0 },
      { elapsed: 1.0 },
      { elapsed: 10.0 }, // 9s 间隔 > 2s 阈值
      { elapsed: 11.0 },
    ];
    expect(computeIdleSegments(events)).toEqual([
      { start: 1.0, end: 10.0 },
    ]);
  });

  test('多个 idle 段', () => {
    const events = [
      { elapsed: 0.0 },
      { elapsed: 5.0 }, // gap 5s
      { elapsed: 6.0 }, // gap 1s 不算
      { elapsed: 100.0 }, // gap 94s
      { elapsed: 101.0 },
    ];
    expect(computeIdleSegments(events)).toEqual([
      { start: 0.0, end: 5.0 },
      { start: 6.0, end: 100.0 },
    ]);
  });

  test('阈值边界', () => {
    expect(IDLE_THRESHOLD).toBe(2.0);
    const events = [
      { elapsed: 0.0 },
      { elapsed: 2.0 }, // 刚好 = 2.0 不算
      { elapsed: 4.01 }, // > 2.0 算
    ];
    expect(computeIdleSegments(events)).toEqual([
      { start: 2.0, end: 4.01 },
    ]);
  });
});