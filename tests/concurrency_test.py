#!/usr/bin/env python3
"""
多 cli 并发测试 — 验证 per-pane 锁正确性
- 同 pane: 5 个并发 cli 应该串行, 5 条命令的 output 都正确, commands.jsonl 行数对
- 不同 pane: 不阻塞 (但需要多个真实 host, 这里只测同 pane)
"""
import concurrent.futures
import json
import subprocess
import time

SSHOPS = "./rust/target/release/sshops-rs"
import os as _os
HOST = _os.environ.get("BENCH_HOST", "@<set BENCH_HOST env, e.g. @aws/edge>")


def fire(cmd_id):
    cmd = f"echo concurrent-{cmd_id}"
    t0 = time.perf_counter()
    r = subprocess.run(
        [SSHOPS, "run", HOST, cmd, "--timeout", "30"],
        capture_output=True, text=True, timeout=60,
    )
    t1 = time.perf_counter()
    try:
        d = json.loads(r.stdout.strip().split("\n")[-1])
        return {
            "id": cmd_id,
            "wall_ms": (t1 - t0) * 1000,
            "server_ms": d["duration_ms"],
            "output": d["output"],
            "session_id": d["session_id"],
        }
    except Exception as e:
        return {"id": cmd_id, "ERR": str(e), "stdout": r.stdout[:200]}


def section(name):
    print()
    print(f"━━━ {name} ━━━")


# 1. 5 个并发 cli 同 pane
section("5 个并发 cli 同 pane (期望串行)")
N = 5
t0 = time.perf_counter()
with concurrent.futures.ThreadPoolExecutor(max_workers=N) as ex:
    futures = [ex.submit(fire, i) for i in range(N)]
    results = [f.result() for f in concurrent.futures.as_completed(futures)]
t1 = time.perf_counter()

results.sort(key=lambda r: r["id"])
for r in results:
    if "ERR" in r:
        print(f"  #{r['id']}: ERR {r['ERR']}  stdout={r['stdout']!r}")
    else:
        out_clean = r["output"].split("\n")[-1] if "\n" in r["output"] else r["output"]
        ok = f"concurrent-{r['id']}" in r["output"]
        print(f"  #{r['id']}: wall={r['wall_ms']:>5.0f}ms server={r['server_ms']:>4}ms  contains_id={'✓' if ok else '✗'}  output_tail={out_clean!r}")

walls = [r.get("wall_ms", 0) for r in results if "wall_ms" in r]
servers = [r.get("server_ms", 0) for r in results if "server_ms" in r]
print()
print(f"  WALL    总耗时 {(t1-t0)*1000:>5.0f}ms  (5 cli 全完成的 wall)")
print(f"          单个 wall 区间 [{min(walls):.0f}, {max(walls):.0f}]")
print(f"  SERVER  单个 server 区间 [{min(servers)}, {max(servers)}]ms")
print(f"          server 之和 = {sum(servers):>4}ms (期望接近总耗时, 串行)")
all_correct = all(f"concurrent-{r['id']}" in r.get("output", "") for r in results)
print(f"  正确性: 所有 5 条命令 output 都含自己 id = {'✓ PASS' if all_correct else '✗ FAIL'}")

# 2. 检查 commands.jsonl 是否记录全
section("commands.jsonl 记录完整性")
sid = results[0].get("session_id") if results else None
if sid:
    import os
    # 找 cast dir (从脚本位置推 repo 根)
    base = os.path.join(
        os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
        "vedio",
    )
    found = None
    for proj in os.listdir(base):
        cand = os.path.join(base, proj, sid)
        if os.path.isdir(cand):
            found = cand
            break
    if found:
        with open(os.path.join(found, "commands.jsonl")) as f:
            lines = f.readlines()
        # 找最近 N 条 ai concurrent-* 命令
        recent_ai = [json.loads(l) for l in lines if "concurrent-" in l]
        print(f"  commands.jsonl 中找到 {len(recent_ai)} 条 concurrent-* 记录 (期望 ≥ {N})")
        # 检查 meta.json 计数
        with open(os.path.join(found, "meta.json")) as f:
            meta = json.load(f)
        print(f"  meta.json: command_count={meta['command_count']}  ai={meta['ai_command_count']}  human={meta['human_command_count']}")
    else:
        print(f"  ⚠ 找不到 cast dir for sid={sid}")
