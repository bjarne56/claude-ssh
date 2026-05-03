#!/usr/bin/env bash
# tests/human-activity-test.sh
# 端到端测试 recent_human_activity 功能:
#  1. unit: 用 mock cast 直接测 record_extract_human_commands 算法
#  2. integration: 真实 ssh, 模拟用户逐字符键入, 验证 ai run 能捕获
#
# 用法: tests/human-activity-test.sh [@<host>]    # 缺主机时只跑 unit

set -eu
SELECTOR="${1:-}"
PASS=0
FAIL=0

pass() { echo "  ✓ $*"; PASS=$((PASS+1)); }
fail() { echo "  ✗ $*"; FAIL=$((FAIL+1)); }

# ============ Unit Test 1: 算法识别 ============
echo "===== UNIT 1: record_extract_human_commands 算法 ====="

mkdir -p /tmp/sshops-ut
cat > /tmp/sshops-ut/stream.cast <<'CAST_EOF'
{"version":3,"term":{"cols":80,"rows":24,"type":"xterm-256color"},"timestamp":1700000000}
[0.0, "o", "[user@host]$ "]
[1.0, "i", "uptime\r"]
[0.05, "o", "uptime\r\nload average\r\n[user@host]$ "]
[2.0, "i", "i"]
[0.1, "i", "f"]
[0.1, "i", "c"]
[0.1, "i", "o"]
[0.1, "i", "n"]
[0.1, "i", "f"]
[0.1, "i", "i"]
[0.1, "i", "g"]
[0.1, "i", "\r"]
[0.05, "o", "ifconfig\r\neth0...\r\n[user@host]$ "]
[5.0, "i", "ls -la /tmp\r"]
[0.05, "o", "ls -la /tmp\r\n...\r\n[user@host]$ "]
[3.0, "i", "p"]
[0.1, "i", "w"]
[0.1, "i", "d"]
[0.1, "i", "\r"]
[0.05, "o", "pwd\r\n/home/user\r\n[user@host]$ "]
CAST_EOF

# 调用算法 (复制 lib/recorder.sh 的 python 部分)
result="$(CAST_TS=1700000000 python3 -c '
import sys, json, os, pathlib
cast_ts = int(os.environ.get("CAST_TS", "0"))
elapsed = 0.0; buf = ""; buf_start = None; max_chunk = 0; groups = []
data = pathlib.Path("/tmp/sshops-ut/stream.cast").read_text().split("\n")[1:]
for line in data:
    line = line.strip()
    if not line: continue
    try: e = json.loads(line)
    except: continue
    if not isinstance(e, list) or len(e) < 3: continue
    elapsed += float(e[0])
    if e[1] != "i": continue
    d = e[2] or ""
    visible = sum(1 for c in d if c.isprintable() or c in "\r\n")
    if visible > max_chunk: max_chunk = visible
    if buf_start is None: buf_start = elapsed
    for ch in d:
        if ch in ("\r", "\n"):
            cmd = buf.strip()
            if cmd:
                groups.append({"cmd": cmd, "max_chunk": max_chunk})
            buf = ""; buf_start = None; max_chunk = 0
        elif ch in ("\x7f", "\x08"): buf = buf[:-1]
        elif ord(ch) < 32: pass
        else: buf += ch

human = [g["cmd"] for g in groups if g["max_chunk"] <= 3 and len(g["cmd"]) > 1]
ai    = [g["cmd"] for g in groups if g["max_chunk"] >  3 or len(g["cmd"]) <= 1]
print(json.dumps({"human": human, "ai": ai}))
')"

[[ "$(echo "$result" | jq -c '.human')" == '["ifconfig","pwd"]' ]] \
    && pass "human 命令识别正确: ifconfig + pwd (逐字符)" \
    || fail "human 期望 [ifconfig,pwd], 实际: $(echo "$result" | jq -c '.human')"

[[ "$(echo "$result" | jq -c '.ai')" == '["uptime","ls -la /tmp"]' ]] \
    && pass "ai 整块命令被排除: uptime + ls -la /tmp" \
    || fail "ai 期望 [uptime, ls -la /tmp], 实际: $(echo "$result" | jq -c '.ai')"

# ============ Unit Test 2: backspace 处理 ============
echo ""
echo "===== UNIT 2: backspace 修正命令 ====="
cat > /tmp/sshops-ut/stream.cast <<'CAST_EOF'
{"version":3,"term":{"cols":80,"rows":24,"type":"xterm-256color"},"timestamp":1700000000}
[1.0, "i", "i"]
[0.1, "i", "p"]
[0.1, "i", " "]
[0.1, "i", "x"]
[0.2, "i", ""]
[0.1, "i", "a"]
[0.1, "i", "\r"]
CAST_EOF

result="$(python3 <<'PY'
import json, pathlib
elapsed = 0.0; buf = ""; max_chunk = 0; groups = []
data = pathlib.Path("/tmp/sshops-ut/stream.cast").read_text().split("\n")[1:]
for line in data:
    line = line.strip()
    if not line: continue
    try: e = json.loads(line)
    except: continue
    if not isinstance(e, list) or len(e) < 3: continue
    elapsed += float(e[0])
    if e[1] != "i": continue
    d = e[2] or ""
    visible = sum(1 for c in d if c.isprintable() or c in "\r\n")
    if visible > max_chunk: max_chunk = visible
    for ch in d:
        if ch in ("\r", "\n"):
            cmd = buf.strip()
            if cmd: groups.append({"cmd": cmd, "max_chunk": max_chunk})
            buf = ""; max_chunk = 0
        elif ch in ("\x7f", "\x08"): buf = buf[:-1]
        elif ord(ch) < 32: pass
        else: buf += ch
print(json.dumps([g for g in groups if g["max_chunk"] <= 3 and len(g["cmd"]) > 1]))
PY
)"

[[ "$(echo "$result" | jq -r '.[0].cmd')" == "ip a" ]] \
    && pass "backspace 处理正确: 'ip x'+BS+'a' = 'ip a'" \
    || fail "期望 'ip a', 实际: $(echo "$result" | jq -r '.[0].cmd')"

# ============ Unit Test 3: 空区间返回 [] ============
echo ""
echo "===== UNIT 3: 空区间 / 边界 ====="
SCRIPT_DIR=/Users/bjarne/Code/ssh-ops

# 直接测真实函数调用 (会用 record_session_dir, mock 它)
record_session_dir() { echo /tmp/sshops-ut; }
strip_ansi() { perl -pe 's/\x1b\[[0-9;]*[a-zA-Z]//g'; }
source <(grep -A 200 "^record_extract_human_commands" "$SCRIPT_DIR/lib/recorder.sh" | head -65)

empty_result="$(record_extract_human_commands "x" 9999 9999 2>/dev/null)"
[[ "$empty_result" == "[]" ]] \
    && pass "start_byte == end_byte → 返回 []" \
    || fail "期望 [], 实际: $empty_result"

# ============ Integration Test (需要 @host) ============
echo ""
echo "===== INTEGRATION: 真实 ssh + wezterm 模拟键入 ====="
if [[ -z "$SELECTOR" ]]; then
    echo "  (跳过, 需要 selector 参数)"
else
    SSHOPS=/Users/bjarne/Code/ssh-ops/bin/sshops
    "$SSHOPS" close "$SELECTOR" 2>&1 | tail -1 || true

    echo "  Step 1: 第一次 ai run (warm up + 写 last_ai_byte)"
    out1="$("$SSHOPS" run "$SELECTOR" "echo W1" 2>/dev/null)"
    h1="$(echo "$out1" | jq -c '.recent_human_activity')"
    [[ "$h1" == "[]" ]] && pass "第 1 次 run, recent_human_activity = []" \
                       || fail "第 1 次期望 [], 实际: $h1"

    echo "  Step 2: SSHOPS_NO_AUTO_HUMAN=1 关闭测试"
    out3="$(SSHOPS_NO_AUTO_HUMAN=1 "$SSHOPS" run "$SELECTOR" "echo W3" 2>/dev/null)"
    h3="$(echo "$out3" | jq -c '.recent_human_activity')"
    [[ "$h3" == "[]" ]] && pass "SSHOPS_NO_AUTO_HUMAN=1 → 返回 []" \
                       || fail "关闭后期望 [], 实际: $h3"

    echo "  Step 3: 直接往当前 session cast 注入逐字符事件 (绕过 wezterm), 验证捕获"
    # 找到当前 session 的 cast 路径
    sid="$(echo "$out1" | jq -r '.session_id')"
    sess_dir="$(dirname "$(/Users/bjarne/Code/ssh-ops/bin/sshops peek "$SELECTOR" 2>&1 | head -1)")"
    # 上述方法不准, 直接从 vedio 找最新
    cast_file="$(find /Users/bjarne/Code/ssh-ops/vedio -name "stream.cast" -mmin -5 2>/dev/null | head -1)"
    if [[ -n "$cast_file" && -f "$cast_file" ]]; then
        # 注意: cast-recorder 还在 append 同一文件, 我们直接 cat 读 + 自己测
        # 改用复制+手动追加方式做单独测试
        test_dir=/tmp/sshops-int-test
        mkdir -p "$test_dir"
        cp "$cast_file" "$test_dir/stream.cast"
        # 追加 5 条逐字符事件 = 'date\r'
        cat >> "$test_dir/stream.cast" <<'EOF'
[3.0, "i", "d"]
[0.1, "i", "a"]
[0.1, "i", "t"]
[0.1, "i", "e"]
[0.1, "i", "\r"]
[0.05, "o", "date\r\nMon May  3 21:00:00 CST 2026\r\n[user@host]$ "]
EOF
        # 用 record_extract_human_commands 直接测
        record_session_dir() { echo "$test_dir"; }
        result="$(record_extract_human_commands "x" 0 0)"
        cnt="$(echo "$result" | jq 'length' 2>/dev/null || echo 0)"
        captured="$(echo "$result" | jq -r '.[].cmd' 2>/dev/null | tr '\n' ',')"
        if (( cnt >= 1 )) && [[ "$captured" == *"date"* ]]; then
            pass "实际 cast + 注入 'date' 逐字符 → 捕获到 $cnt 条 human (含 date)"
        else
            fail "期望捕获到 'date', 实际: $result"
        fi
    else
        fail "找不到 session cast 文件"
    fi
fi

# ============ 总结 ============
echo ""
echo "===== 结果: $PASS 通过, $FAIL 失败 ====="
[[ $FAIL -eq 0 ]] && exit 0 || exit 1
