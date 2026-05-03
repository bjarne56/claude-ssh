#!/usr/bin/env python3
"""
全链路场景 benchmark — 测大数据输出 / 慢命令(回调) / 边缘 case
跑前需有 daemon 在运行 + 目标 pane 已 open
"""
import json
import statistics
import subprocess
import time

SSHOPS = "./rust/target/release/sshops-rs"
HOST = "@10.32.49.7"


def run(cmd: str, timeout: int = 30) -> dict:
    t0 = time.perf_counter()
    r = subprocess.run(
        [SSHOPS, "run", HOST, cmd, "--timeout", str(timeout)],
        capture_output=True,
        text=True,
        timeout=timeout + 5,
    )
    t1 = time.perf_counter()
    wall = (t1 - t0) * 1000
    try:
        d = json.loads(r.stdout.strip().split("\n")[-1])
        d["__wall_ms"] = wall
        d["__output_bytes"] = len(d.get("output", ""))
        d["__output_lines"] = d.get("output", "").count("\n") + 1 if d.get("output") else 0
        return d
    except Exception as e:
        return {"__error": str(e), "__stdout": r.stdout[:200], "__wall_ms": wall}


def fmt(d: dict) -> str:
    if "__error" in d:
        return f"ERR {d['__error']}"
    return (
        f"wall={d['__wall_ms']:>6.0f}ms  "
        f"server={d.get('duration_ms', 0):>5}ms  "
        f"out={d['__output_bytes']:>7}B/{d['__output_lines']:>5}行  "
        f"exit={d.get('exit', '?')}"
    )


def section(name: str):
    print()
    print(f"━━━ {name} ━━━")


def case(label: str, cmd: str, timeout: int = 30):
    d = run(cmd, timeout)
    print(f"  {label:<35} {fmt(d)}")
    return d


# ============================================================
# 1. 大数据输出
# ============================================================
section("大数据输出(单次)")
case("seq 100  (~390B)", "seq 100")
case("seq 1000 (~4KB)", "seq 1000")
case("seq 10000 (~50KB)", "seq 10000")
case("seq 100000 (~600KB)", "seq 100000", timeout=60)
case("dmesg (真实日志)", "dmesg | head -200")
case("find /etc -type f (~1MB)", "find /etc -type f 2>/dev/null", timeout=60)
case("base64 of 100KB random", "head -c 100000 /dev/urandom | base64")
case("ls -lR /usr/include  (~MB)", "ls -lR /usr/include 2>/dev/null | head -5000", timeout=60)

# ============================================================
# 2. 慢命令(回调/分批输出)
# ============================================================
section("慢命令(回调/分批输出)")
case("sleep 1 + echo", "sleep 1 && echo done")
case("sleep 3 + echo", "sleep 3 && echo done")
case("for sleep 5x1s 分批 echo", "for i in 1 2 3 4 5; do echo $i; sleep 1; done")
case("ping -c 3 127.0.0.1", "ping -c 3 127.0.0.1")
case("find / -name xxx 2>/dev/null", "timeout 5 find / -name 'xxx-not-exist' 2>/dev/null; echo done", timeout=15)

# ============================================================
# 3. 边缘 case
# ============================================================
section("边缘 case")
case("空输出", "true")
case("只有 stderr", "echo err >&2")
case("大量 ANSI 颜色", "ls --color=always /usr 2>/dev/null | head -50")
case("含特殊字符", "echo 'hello \"world\" $USER \\\\ `date +%Y`'")
case("非零 exit 码", "false; echo $?")
case("不存在文件", "cat /nonexistent-file-xyz 2>&1; echo done")
case("中文输出", "echo '你好世界 — Hello'")
case("长单行 (no \\n)", "printf 'x%.0s' {1..1000}; echo")

# ============================================================
# 4. 重复跑同一命令统计稳定性
# ============================================================
section("稳定性 (echo × 10 次)")
walls = []
servers = []
for i in range(10):
    d = run(f"echo stab-{i}")
    walls.append(d["__wall_ms"])
    servers.append(d.get("duration_ms", 0))
print(
    f"  WALL    avg={statistics.mean(walls):>6.0f}ms  "
    f"min={min(walls):>4.0f}  max={max(walls):>4.0f}  "
    f"p50={statistics.median(walls):>4.0f}  stdev={statistics.stdev(walls):>4.0f}"
)
print(
    f"  SERVER  avg={statistics.mean(servers):>6.0f}ms  "
    f"min={min(servers):>4.0f}  max={max(servers):>4.0f}  "
    f"p50={statistics.median(servers):>4.0f}  stdev={statistics.stdev(servers):>4.0f}"
)

# ============================================================
# 5. 大数据 × 5 次 (吞吐稳定性)
# ============================================================
section("大数据稳定性 (seq 10000 × 5 次)")
walls = []
sizes = []
for i in range(5):
    d = run("seq 10000")
    walls.append(d["__wall_ms"])
    sizes.append(d.get("__output_bytes", 0))
print(
    f"  WALL  avg={statistics.mean(walls):>6.0f}ms  "
    f"min={min(walls):>5.0f}  max={max(walls):>5.0f}"
)
print(f"  SIZE  avg={statistics.mean(sizes):>7.0f}B (期望 ~48894B)")
print(f"        all_match: {all(s == sizes[0] for s in sizes)}")

# ============================================================
# 6. recent_human_activity 回调
# ============================================================
section("recent_human_activity 回调")
d = run("echo cb-test")
print(f"  最近 human 活动数: {len(d.get('recent_human_activity', []))}")
if d.get("recent_human_activity"):
    print(f"  样例: {d['recent_human_activity'][0]}")
