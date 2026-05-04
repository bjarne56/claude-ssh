#!/usr/bin/env bash
# install.sh — ssh-ops 安装入口 (幂等).
#
# 流程:
#   1. 系统依赖自检 (asciinema / jq / ssh / cargo 等)
#   2. (macOS) 构建 + 部署 wezterm-src fork → ~/Applications/WezTerm-SSH.app
#      产出 WezTerm-SSH (GUI) / WezTerm-SSH-cli / WezTerm-SSH-mux
#   3. 部署 skill 到 ~/.claude/skills/sshops/ (按系统 locale 选 SKILL.md 语言)
#      旧 ~/.claude/skills/ssh-ops 自动清理
#   4. 写入 PATH 到 ~/.zshenv (~/Code/ssh-ops/bin + ~/.local/bin)
#
# 选项:
#   --no-build-wezterm   跳过 wezterm-src 构建/部署 (假设已部署或非 macOS)
#   --link-only          只部署 skill, 跳过依赖检查 + wezterm 构建
#   --locale <code>      强制使用某个 locale (zh-CN / en / 等), 默认从系统检测
#   -h | --help          打印帮助
#
# 幂等: 反复跑只会跳过已就位的步骤, 不会报错或重复写入.
set -euo pipefail

SSHOPS_SRC="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SSHOPS_DEST="$HOME/.claude/skills/sshops"   # skill 目录名 = slash 命令名 (sshops)
SSHOPS_OLD_DEST="$HOME/.claude/skills/ssh-ops"  # 旧目录名, install 时自动清理
WEZ_SRC="$SSHOPS_SRC/wezterm-src"
SKILL_LOCALES_DIR="$SSHOPS_SRC/skill-locales"

opt_no_build_wezterm=0
opt_link_only=0
opt_locale=""
while (( $# > 0 )); do
    case "$1" in
        --no-build-wezterm) opt_no_build_wezterm=1; shift ;;
        --link-only)        opt_link_only=1; shift ;;
        --locale)           opt_locale="${2:-}"; shift 2 ;;
        -h|--help)
            sed -n '2,/^set -euo/p' "$0" | sed '$d; s/^# *//; s/^#//'
            exit 0
            ;;
        *) shift ;;
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
echo "==> 检测 locale 并选 SKILL.md 翻译"

# detect_locale: 优先 --locale 参数, 否则 macOS AppleLocale, 否则 LANG
detect_locale() {
    if [[ -n "$opt_locale" ]]; then
        printf '%s' "$opt_locale"
        return
    fi
    if [[ "$(uname)" == "Darwin" ]]; then
        local apple
        apple="$(defaults read -g AppleLocale 2>/dev/null || true)"
        # zh-Hans_MY → zh-Hans, en_US → en
        printf '%s' "${apple%%_*}"
        return
    fi
    # LANG=zh_CN.UTF-8 → zh_CN; LANG=C → ""
    local lang="${LANG%%.*}"
    [[ "$lang" == "C" || "$lang" == "POSIX" ]] && lang=""
    printf '%s' "$lang"
}

# resolve_skill_file <locale> -> 输出选中文件路径; 找不到 fallback 到 SKILL.en.md
# 查找顺序:
#   1. skill-locales/SKILL.<locale>.md (精确 zh-CN / pt-BR)
#   2. skill-locales/SKILL.<lang>.md (前缀 zh / pt)
#   3. SKILL.md (项目根, 默认 zh-CN)
#   4. skill-locales/SKILL.en.md (最终 fallback)
resolve_skill_file() {
    local loc="$1"
    # zh-Hans / zh-CN / zh-Hant 都规一到 zh-CN(简体) 或 zh-TW(繁体)
    case "$loc" in
        zh-Hans|zh|zh-CN|zh_CN) loc="zh-CN" ;;
        zh-Hant|zh-TW|zh_TW|zh-HK|zh_HK) loc="zh-TW" ;;
    esac
    # zh-CN 是项目主语言, 用根目录 SKILL.md
    if [[ "$loc" == "zh-CN" && -f "$SSHOPS_SRC/SKILL.md" ]]; then
        printf '%s' "$SSHOPS_SRC/SKILL.md"
        return
    fi
    # 精确文件
    if [[ -f "$SKILL_LOCALES_DIR/SKILL.$loc.md" ]]; then
        printf '%s' "$SKILL_LOCALES_DIR/SKILL.$loc.md"
        return
    fi
    # 语言前缀 fallback (en-GB → en)
    local prefix="${loc%%-*}"
    if [[ -n "$prefix" && "$prefix" != "$loc" && -f "$SKILL_LOCALES_DIR/SKILL.$prefix.md" ]]; then
        printf '%s' "$SKILL_LOCALES_DIR/SKILL.$prefix.md"
        return
    fi
    # 最终 fallback: 英文
    printf '%s' "$SKILL_LOCALES_DIR/SKILL.en.md"
}

LOCALE="$(detect_locale)"
[[ -z "$LOCALE" ]] && LOCALE="en"
SKILL_FILE="$(resolve_skill_file "$LOCALE")"
ok "system locale: $LOCALE → $(basename "$SKILL_FILE")"

echo
echo "==> 部署 skill 到 $SSHOPS_DEST"

# 旧目录名 ~/.claude/skills/ssh-ops/ (slash 命令是 /ssh-ops) 已重命名为 sshops, 自动清理
if [[ -L "$SSHOPS_OLD_DEST" || -d "$SSHOPS_OLD_DEST" ]]; then
    rm -rf "$SSHOPS_OLD_DEST"
    warn "已清理旧 skill 目录: $SSHOPS_OLD_DEST (slash 命令从 /ssh-ops → /sshops)"
fi

# sshops 目录策略: 实体目录 + symlink, 让 SKILL.md 可独立按 locale 切换
# 老的 symlink-到-整个仓库 方式不允许 SKILL.md 跟仓库分离, 改不了语言.
mkdir -p "$SSHOPS_DEST"

# SKILL.md: cp (按 locale 选定的版本, 不是 ln, 因为下次换 locale 要重 cp)
cp -f "$SKILL_FILE" "$SSHOPS_DEST/SKILL.md"
ok "SKILL.md ← $(basename "$SKILL_FILE")"

# 其他业务文件: ln -s 到源仓库 (省得每次同步)
declare -a SKILL_LINKS=(bin lib rust state tests config.example.json README.md docs ssh-ops-requirements.md)
for item in "${SKILL_LINKS[@]}"; do
    src="$SSHOPS_SRC/$item"
    dst="$SSHOPS_DEST/$item"
    [[ ! -e "$src" ]] && continue
    if [[ -L "$dst" ]]; then
        cur="$(readlink "$dst")"
        if [[ "$cur" == "$src" ]]; then
            continue   # 幂等: 已正确链接
        fi
        rm -f "$dst"
    elif [[ -e "$dst" ]]; then
        warn "  $dst 已存在且非 symlink, 跳过"
        continue
    fi
    ln -s "$src" "$dst"
done
ok "业务文件 (bin/ lib/ rust/ ...) 已链接到源仓库"

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
echo
echo "Claude Code 中调用:  /sshops <你的请求>  (slash 命令名跟 SKILL frontmatter 一致)"
echo "切换 SKILL 语言:    bash install.sh --link-only --locale en  (默认从系统检测)"
