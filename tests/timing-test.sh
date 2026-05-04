#!/usr/bin/env bash
# tests/timing-test.sh
# 测 sshops run 完整链路各阶段耗时, 找瓶颈
#
# 用法:
#   tests/timing-test.sh <selector>             # 用已配置主机
#   tests/timing-test.sh @aws/edge
#   SSHOPS_POLL_INTERVAL=0.1 tests/timing-test.sh ...
#
# 跑 5 类命令各 3 次, 输出每阶段耗时表格

set -eu

SELECTOR="${1:-}"
if [[ -z "$SELECTOR" ]]; then
    echo "用法: $0 <selector>"
    echo "例: $0 @aws/edge   或   $0 cvm-01"
    exit 1
fi

SSHOPS_BIN="$(dirname "$0")/../bin/sshops"
[[ -x "$SSHOPS_BIN" ]] || { echo "找不到 $SSHOPS_BIN"; exit 1; }

# 测试用例: (name, command)
declare -a TESTS=(
    "tiny|true"                                   # 几乎零输出
    "small|echo 'hello world'"                    # 1 行
    "medium|seq 1 100"                            # 100 行
    "large|seq 1 10000"                           # 10000 行 (~50KB)
    "binary|head -c 200000 /dev/urandom | base64" # 大量乱码
)

ROUNDS=3

echo "===== sshops 链路计时测试 ====="
echo "selector: $SELECTOR"
echo "rounds:   $ROUNDS"
echo "poll_interval: ${SSHOPS_POLL_INTERVAL:-0.2}s"
echo ""
printf "%-10s %-8s %-10s %-10s %-12s %-10s %-10s\n" \
    "name" "round" "send_ms" "poll_ms" "cast_ms" "total_ms" "out_bytes"
echo "------------------------------------------------------------------------------"

for tc in "${TESTS[@]}"; do
    name="${tc%%|*}"
    cmd="${tc#*|}"
    for ((r=1; r<=ROUNDS; r++)); do
        # 启用 timing debug, 让 sshops 把各阶段输出到 stderr
        out="$(SSHOPS_DEBUG_TIMING=1 "$SSHOPS_BIN" run "$SELECTOR" "$cmd" 2> /tmp/_ts_stderr || true)"
        timing_line="$(grep "TIMING:" /tmp/_ts_stderr 2>/dev/null | tail -1)"

        send_ms="$(echo "$timing_line" | grep -oE 'send=[0-9]+' | tr -d 'send=' || echo 0)"
        poll_ms="$(echo "$timing_line" | grep -oE 'poll=[0-9]+' | tr -d 'poll=' || echo 0)"
        cast_ms="$(echo "$timing_line" | grep -oE 'cast_extract=[0-9]+' | tr -d 'cast_extra=' || echo 0)"
        total_ms="$(echo "$timing_line" | grep -oE 'total=[0-9]+' | tr -d 'total=' || echo 0)"

        # 取 output 字节数 (jq 解析)
        out_bytes="$(echo "$out" | jq -r '.output | length' 2>/dev/null || echo "?")"

        printf "%-10s %-8s %-10s %-10s %-12s %-10s %-10s\n" \
            "$name" "$r" "$send_ms" "$poll_ms" "$cast_ms" "$total_ms" "$out_bytes"
    done
done

echo ""
echo "瓶颈分析提示:"
echo "  - send_ms 高    → wt_send_text (PTY 写入) 慢"
echo "  - poll_ms 高    → 等 prompt 太久 (调小 SSHOPS_POLL_INTERVAL)"
echo "  - cast_ms 高    → cast 文件解析慢 (大输出 awk 处理)"
echo "  - 命令本身耗时也含在 poll_ms 里 (无法分离, 但用 'true' 测可见 baseline)"
