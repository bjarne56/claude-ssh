#!/usr/bin/env bash
# tests/full-chain-benchmark.sh
# 全链路速度测试: 多场景 + 各阶段耗时分布 + 关闭/开启 recent_human_activity 对比
#
# 用法: tests/full-chain-benchmark.sh @<host>

set -eu
SELECTOR="${1:-}"
[[ -z "$SELECTOR" ]] && { echo "用法: $0 @<host>"; exit 1; }

SSHOPS=/Users/bjarne/Code/ssh-ops/bin/sshops
ROUNDS=3

now_ms() {
    if command -v gdate >/dev/null 2>&1; then
        gdate '+%s%N' | awk '{print int($0/1000000)}'
    else
        python3 -c 'import time; print(int(time.time()*1000))'
    fi
}

# 跑一个场景 N 轮, 输出 中位数 (各阶段)
run_scenario() {
    local label="$1" cmd="$2" extra_env="${3:-}"
    local -a walls sends polls casts
    for ((r=1; r<=ROUNDS; r++)); do
        local t0 t1 wall_ms
        t0=$(now_ms)
        # shellcheck disable=SC2086
        env $extra_env SSHOPS_DEBUG_TIMING=1 "$SSHOPS" run "$SELECTOR" "$cmd" \
            >/tmp/_bench_out 2>/tmp/_bench_err || true
        t1=$(now_ms)
        wall_ms=$((t1 - t0))

        local timing; timing="$(grep "TIMING:" /tmp/_bench_err | tail -1)"
        local send poll cast
        send="$(echo "$timing" | grep -oE 'send=[0-9]+' | tr -d 'send=')"
        poll="$(echo "$timing" | grep -oE 'poll=[0-9]+' | tr -d 'poll=')"
        cast="$(echo "$timing" | grep -oE 'cast_extract=[0-9]+' | tr -d 'cast_extra=')"

        walls+=("$wall_ms")
        sends+=("${send:-0}")
        polls+=("${poll:-0}")
        casts+=("${cast:-0}")
    done

    local m_wall m_send m_poll m_cast
    m_wall=$(printf '%s\n' "${walls[@]}" | sort -n | awk -v n=$ROUNDS 'NR==int(n/2)+1')
    m_send=$(printf '%s\n' "${sends[@]}" | sort -n | awk -v n=$ROUNDS 'NR==int(n/2)+1')
    m_poll=$(printf '%s\n' "${polls[@]}" | sort -n | awk -v n=$ROUNDS 'NR==int(n/2)+1')
    m_cast=$(printf '%s\n' "${casts[@]}" | sort -n | awk -v n=$ROUNDS 'NR==int(n/2)+1')
    local overhead=$((m_wall - m_send - m_poll - m_cast))

    printf "  %-30s %-7s %-7s %-7s %-7s %-7s\n" \
        "$label" "$m_wall" "$overhead" "$m_send" "$m_poll" "$m_cast"
}

echo "===== ssh-ops 全链路速度 benchmark ====="
echo "selector: $SELECTOR  rounds/scenario: $ROUNDS"
echo ""

# 预热: 第一次会触发登录 (38s)
echo "预热 (含登录)..."
"$SSHOPS" close "$SELECTOR" 2>&1 | tail -1 || true
"$SSHOPS" run "$SELECTOR" "true" >/dev/null 2>&1 || true
echo ""

printf "  %-30s %-7s %-7s %-7s %-7s %-7s\n" \
    "scenario" "wall" "ovr" "send" "poll" "cast"
echo "  ──────────────────────────────────────────────────────────────────"

# ========== 场景 A: 不同输出大小 (默认含 recent_human) ==========
run_scenario "A1: tiny (true)"            "true"
run_scenario "A2: small (echo hi)"        "echo hi"
run_scenario "A3: medium (seq 100)"       "seq 1 100"
run_scenario "A4: large (seq 10000)"      "seq 1 10000"
run_scenario "A5: huge binary (200KB)"    "head -c 200000 /dev/urandom | base64"

echo ""
echo "  ────── B: SSHOPS_NO_AUTO_HUMAN=1 (关闭 human 感知, 看节省多少) ──────"
run_scenario "B1: tiny  (no_human)"        "true"               "SSHOPS_NO_AUTO_HUMAN=1"
run_scenario "B2: small (no_human)"        "echo hi"            "SSHOPS_NO_AUTO_HUMAN=1"
run_scenario "B3: large (no_human)"        "seq 1 10000"        "SSHOPS_NO_AUTO_HUMAN=1"

echo ""
echo "===== 字段说明 ====="
echo "  wall = 调用→返回 总墙钟时间 (含 bash 启动)"
echo "  ovr  = overhead = wall - (send+poll+cast), 主要是 bash 启动 + 选择器解析 + recent_human + JSON 序列化"
echo "  send = wezterm cli send-text PTY 注入"
echo "  poll = 等 prompt 出现 (cast tail)"
echo "  cast = 等 cast flush 稳定 + jq 切片"
echo ""
echo "===== AI 端到端理论延迟 ====="
echo "  Anthropic API 单 round-trip: ~100-200ms"
echo "  AI tool 调用 sshops: wall 上面 + ~100-200ms"
echo "  典型: 短命令 ~1.3s, 大输出 ~1.5s (含 AI 通信)"
