#!/usr/bin/env bash
# tests/e2e-timing-test.sh
# 端到端链路计时: AI 调 Skill → bash 启动 sshops → 命令执行 → 返回 JSON
#
# 测每个阶段的延迟, 找瓶颈
#
# 用法: tests/e2e-timing-test.sh @<host>

set -eu

SELECTOR="${1:-}"
if [[ -z "$SELECTOR" ]]; then
    echo "用法: $0 @<host>"
    exit 1
fi

SSHOPS_BIN="$(dirname "$0")/../bin/sshops"
[[ -x "$SSHOPS_BIN" ]] || { echo "找不到 $SSHOPS_BIN"; exit 1; }

# 先确保 pane 是热的 (登录已完成, 测稳态)
echo "===== 预热: 登录 + 初始化 (首次会慢) ====="
SSHOPS_DEBUG_TIMING=1 "$SSHOPS_BIN" run "$SELECTOR" "true" 2>&1 | grep -E "TIMING:|duration_ms" | head -2
echo ""

# ====== 端到端各阶段计时 ======
# 阶段 A: bash 启动 sshops 进程 (CLI overhead)
# 阶段 B: 选择器解析 (parse + securecrt 查找)
# 阶段 C: pane 复用判断 (查 panes.json + wezterm cli list)
# 阶段 D: wt_send_text (wezterm cli send-text PTY 写入)
# 阶段 E: cast tail prompt 检测
# 阶段 F: cast 字节切片 (等 flush + jq 解析)
# 阶段 G: JSON 输出 + record_append_command + 进程退出

# 跑 N 轮取中位数
ROUNDS=5
declare -a totals
declare -a sends polls casts

echo "===== 稳态全链路测试 (echo, 5 轮) ====="
echo ""
printf "%-8s %-10s %-10s %-10s %-12s\n" "round" "wall_ms" "send_ms" "poll_ms" "cast_ms"
echo "----------------------------------------------------------"

for ((r=1; r<=ROUNDS; r++)); do
    # 用 wall clock 计 sshops 进程总耗时 (含 bash startup)
    t_start=$(($(gdate '+%s%N' 2>/dev/null || python3 -c 'import time; print(int(time.time()*1e9))') / 1000000))
    out="$(SSHOPS_DEBUG_TIMING=1 "$SSHOPS_BIN" run "$SELECTOR" "echo R$r" 2>/tmp/_e2e_stderr)"
    t_end=$(($(gdate '+%s%N' 2>/dev/null || python3 -c 'import time; print(int(time.time()*1e9))') / 1000000))
    wall_ms=$((t_end - t_start))

    timing_line="$(grep "TIMING:" /tmp/_e2e_stderr | tail -1)"
    send_ms="$(echo "$timing_line" | grep -oE 'send=[0-9]+' | tr -d 'send=')"
    poll_ms="$(echo "$timing_line" | grep -oE 'poll=[0-9]+' | tr -d 'poll=')"
    cast_ms="$(echo "$timing_line" | grep -oE 'cast_extract=[0-9]+' | tr -d 'cast_extra=')"

    totals+=("$wall_ms")
    sends+=("$send_ms")
    polls+=("$poll_ms")
    casts+=("$cast_ms")

    printf "%-8s %-10s %-10s %-10s %-12s\n" "$r" "$wall_ms" "$send_ms" "$poll_ms" "$cast_ms"
done

# ====== 统计 ======
median() {
    local arr=("$@")
    IFS=$'\n' sorted=($(sort -n <<<"${arr[*]}"))
    unset IFS
    local mid=$((${#sorted[@]} / 2))
    echo "${sorted[$mid]}"
}

avg() {
    local sum=0
    for v in "$@"; do sum=$((sum + v)); done
    echo $((sum / $#))
}

echo ""
echo "===== 阶段耗时统计 (中位数) ====="
m_wall=$(median "${totals[@]}")
m_send=$(median "${sends[@]}")
m_poll=$(median "${polls[@]}")
m_cast=$(median "${casts[@]}")
sshops_internal=$((m_send + m_poll + m_cast))
bash_overhead=$((m_wall - sshops_internal))

printf "  %-30s %d ms\n" "wall (调用→返回 总耗时)" "$m_wall"
printf "  %-30s %d ms  (bash 启动 + 选择器解析 + JSON 输出)\n" "├─ overhead" "$bash_overhead"
printf "  %-30s %d ms  (wezterm cli send-text)\n" "├─ wt_send_text" "$m_send"
printf "  %-30s %d ms  (cast tail 等 prompt)\n" "├─ prompt 检测" "$m_poll"
printf "  %-30s %d ms  (等 flush 稳定 + jq 解析)\n" "└─ cast 切片" "$m_cast"

echo ""
echo "===== 优化方向 ====="
if (( bash_overhead > 200 )); then
    echo "⚠ overhead 高 (${bash_overhead}ms) — bash 解析 secureCRT/路径慢, 可缓存"
fi
if (( m_send > 80 )); then
    echo "⚠ send 高 (${m_send}ms) — wezterm cli fork+exec 开销, 考虑 socket 协议"
fi
if (( m_poll > 200 )); then
    echo "⚠ poll 高 (${m_poll}ms) — cast 还没 flush 命令完成的 prompt, 调 record_wait_prompt poll_ms"
fi
if (( m_cast > 300 )); then
    echo "⚠ cast 高 (${m_cast}ms) — 等 flush 稳定 (3 次 100ms × poll), 大输出 jq 慢"
fi

echo ""
echo "===== AI 调 Skill 全链路 (理论延迟) ====="
echo "  AI 决定 → Tool call:                    ~50-200ms (Anthropic API)"
echo "  Tool exec (bash sshops):                 ${m_wall}ms (本测试)"
echo "  AI 看到结果:                             ~50-200ms (API 返回)"
echo ""
echo "  端到端: ~$((m_wall + 200))-$((m_wall + 400)) ms (含 AI API)"
