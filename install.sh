#!/usr/bin/env bash
# install.sh — ssh-ops 安装入口 (幂等).
#
# 流程:
#   1. 系统依赖自检 (asciinema / jq / ssh / cargo 等)
#   2. (macOS) 构建 + 部署 wezterm-src fork → ~/Applications/WezTerm-SSH.app
#      产出 WezTerm-SSH (GUI) / WezTerm-SSH-cli / WezTerm-SSH-mux
#   3. 链接 ssh-ops skill 到 ~/.claude/skills/ssh-ops/
#   4. 写入 PATH 到 ~/.zshenv (~/Code/ssh-ops/bin + ~/.local/bin)
#
# 选项:
#   --no-build-wezterm   跳过 wezterm-src 构建/部署 (假设已部署或非 macOS)
#   --link-only          只链接 skill, 跳过依赖检查 + wezterm 构建
#   -h | --help          打印帮助
#
# 幂等: 反复跑只会跳过已就位的步骤, 不会报错或重复写入.
set -euo pipefail

SSHOPS_SRC="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SSHOPS_DEST="$HOME/.claude/skills/ssh-ops"
WEZ_SRC="$SSHOPS_SRC/wezterm-src"

opt_no_build_wezterm=0
opt_link_only=0
for arg in "$@"; do
    case "$arg" in
        --no-build-wezterm) opt_no_build_wezterm=1 ;;
        --link-only)        opt_link_only=1 ;;
        -h|--help)
            sed -n '2,/^set -euo/p' "$0" | sed '$d; s/^# *//; s/^#//'
            exit 0
            ;;
    esac
done

ok()   { printf '\033[32m✓\033[0m %s\n' "$*"; }
warn() { printf '\033[33m!\033[0m %s\n' "$*"; }
err()  { printf '\033[31m✗\033[0m %s\n' "$*" >&2; }

check_cmd() {
    # check_cmd <name> <required> <hint>
    local cmd="$1" required="$2" hint="$3"
    if command -v "$cmd" >/dev/null 2>&1; then
        ok "$cmd: $(command -v "$cmd")"
        return 0
    fi
    if [[ "$required" == "1" ]]; then
        err "$cmd 未安装(必需)。安装提示: $hint"
        return 1
    else
        warn "$cmd 未安装(可选)。安装提示: $hint"
        return 0
    fi
}

fail=0

if (( opt_link_only == 0 )); then
    echo "==> 检查系统依赖"
    check_cmd asciinema  1 "macOS: brew install asciinema  /  Linux: pip install asciinema 或 apt install asciinema" || fail=1
    check_cmd jq         1 "brew install jq" || fail=1
    check_cmd ssh        1 "(自带,通常无需安装)" || fail=1
    check_cmd sshpass    0 "macOS: brew install hudochenkov/sshpass/sshpass  /  Linux: apt install sshpass" || true
    check_cmd perl       1 "(自带,通常无需安装)" || fail=1
    check_cmd xxd        1 "(自带,通常无需安装)" || fail=1
    check_cmd pass       0 "macOS: brew install pass  /  Linux: apt install pass" || true
    check_cmd cargo      1 "rustup: https://rustup.rs (构建 wezterm-src fork 需要)" || fail=1

    # bash 版本:本 skill 兼容 bash 3.2 / 4 / 5
    ok "bash: $BASH_VERSION (3.2 / 4 / 5 全部兼容)"

    if (( fail == 1 )); then
        err "有必需依赖缺失,先安装上面提示的工具再重试"
        exit 1
    fi

    # === 构建 + 部署 wezterm-src fork (macOS) ===
    if [[ "$(uname)" == "Darwin" && "$opt_no_build_wezterm" != "1" ]]; then
        echo
        echo "==> 构建并部署 wezterm-src fork → ~/Applications/WezTerm-SSH.app"
        if [[ ! -d "$WEZ_SRC" ]]; then
            err "wezterm-src 目录不存在: $WEZ_SRC"
            err "  本仓库期望 wezterm-src 与 install.sh 同级 (子目录)"
            exit 1
        fi
        # install-local.sh 自身幂等: 已部署只覆盖同名产物, ~/.local/bin wrapper 已存在则覆盖
        bash "$WEZ_SRC/install-local.sh"
        echo
    elif [[ "$opt_no_build_wezterm" == "1" ]]; then
        warn "跳过 wezterm-src 构建 (--no-build-wezterm)"
    fi

    # 验证 WezTerm-SSH-cli 在 PATH (install-local.sh 已链接到 ~/.local/bin/)
    if command -v WezTerm-SSH-cli >/dev/null 2>&1; then
        ok "WezTerm-SSH-cli: $(command -v WezTerm-SSH-cli)"
    elif [[ -x "$HOME/.local/bin/WezTerm-SSH-cli" ]]; then
        warn "WezTerm-SSH-cli 在 ~/.local/bin/ 但当前 PATH 没包含; 启动新 shell 加载 ~/.zshenv 即可"
    else
        err "WezTerm-SSH-cli 不在 PATH 也不在 ~/.local/bin; install-local.sh 失败?"
        exit 1
    fi
else
    warn "--link-only: 跳过依赖检查与 wezterm-src 构建"
fi

echo
echo "==> 链接 skill 到 $SSHOPS_DEST"
mkdir -p "$(dirname "$SSHOPS_DEST")"

# 幂等: 已是正确 symlink 跳过, 错误 symlink 警告, 非 symlink 文件报错
if [[ -L "$SSHOPS_DEST" ]]; then
    cur="$(readlink "$SSHOPS_DEST")"
    if [[ "$cur" == "$SSHOPS_SRC" ]]; then
        ok "已链接: $SSHOPS_DEST → $SSHOPS_SRC"
    else
        warn "已存在 symlink 指向其它路径: $cur"
        warn "  如需重链, 先 rm '$SSHOPS_DEST' 再重跑"
    fi
elif [[ -e "$SSHOPS_DEST" ]]; then
    err "$SSHOPS_DEST 已存在且不是 symlink, 请先备份或移除"
    exit 1
else
    ln -s "$SSHOPS_SRC" "$SSHOPS_DEST"
    ok "已链接: $SSHOPS_DEST → $SSHOPS_SRC"
fi

# 关键脚本可执行权限 (幂等: chmod +x 反复执行无副作用)
for f in bin/sshops bin/sshops-setup install.sh tests/self-test.sh \
         wezterm-src/install-local.sh ; do
    chmod +x "$SSHOPS_SRC/$f" 2>/dev/null || true
done

echo
echo "==> PATH 写入 ~/.zshenv (幂等)"
ZSHRC="$HOME/.zshenv"
declare -a PATH_ENTRIES=(
    'export PATH="$HOME/Code/ssh-ops/bin:$PATH"'
    'export PATH="$HOME/.local/bin:$PATH"'
)
for entry in "${PATH_ENTRIES[@]}"; do
    if grep -qF "$entry" "$ZSHRC" 2>/dev/null; then
        ok "PATH 已在 $ZSHRC: ${entry#export PATH=}"
    elif [[ -w "$ZSHRC" || ! -e "$ZSHRC" ]]; then
        echo "$entry" >> "$ZSHRC"
        ok "PATH 已写入 $ZSHRC: ${entry#export PATH=}"
    else
        warn "无法写入 $ZSHRC, 请手动加: $entry"
    fi
done

echo
echo "==> 下一步"
echo "  1) 启动新 shell (让 ~/.zshenv 的 PATH 生效) 或: source ~/.zshenv"
echo "  2) sshops setup            # 交互式向导写 config.json"
echo "  3) open -a WezTerm-SSH     # 启动 GUI (双击 .app 同效果)"
echo "  4) bash $SSHOPS_SRC/tests/self-test.sh   # 冒烟测试 (需 ssh localhost 可登录)"
