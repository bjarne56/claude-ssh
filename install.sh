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
#   --status             检测当前部署状态 (skill / GUI / CLI / daemon / config) 后退出
#   --uninstall          卸载所有部署 (skill / .app / wrapper); 不动源仓库与录像
#   --yes                跳过卸载确认 (跟 --uninstall 配合, CI 用)
#   --auto-approve       patch ~/.claude/settings.json 把 Bash(sshops:*) 加入
#                        permissions.allow, 让 Claude Code 跑 sshops 命令不再
#                        弹 yes 提示 (备份原文件到 settings.json.bak)
#   -h | --help          打印帮助
#
# 幂等: 反复跑只会跳过已就位的步骤, 不会报错或重复写入.
set -euo pipefail

SSHOPS_SRC="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SSHOPS_DEST="$HOME/.claude/skills/sshops"   # skill 目录名 = slash 命令名 (sshops)
SSHOPS_OLD_DEST="$HOME/.claude/skills/ssh-ops"  # 旧目录名, install 时自动清理
WEZ_SRC="$SSHOPS_SRC/wezterm-src"
SKILL_LOCALES_DIR="$SSHOPS_SRC/skill-locales"
WEZ_APP="$HOME/Applications/WezTerm-SSH.app"
LOCAL_BIN="$HOME/.local/bin"

opt_no_build_wezterm=0
opt_link_only=0
opt_locale=""
opt_status=0
opt_uninstall=0
opt_yes=0
opt_auto_approve=0
while (( $# > 0 )); do
    case "$1" in
        --no-build-wezterm) opt_no_build_wezterm=1; shift ;;
        --link-only)        opt_link_only=1; shift ;;
        --locale)           opt_locale="${2:-}"; shift 2 ;;
        --status)           opt_status=1; shift ;;
        --uninstall)        opt_uninstall=1; shift ;;
        --yes|-y)           opt_yes=1; shift ;;
        --auto-approve)     opt_auto_approve=1; shift ;;
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
inf()  { printf '\033[36m•\033[0m %s\n' "$*"; }
miss() { printf '\033[2m·\033[0m %s\n' "$*"; }   # 灰色: 未安装

# detect_locale: 优先 --locale 参数, 否则 macOS AppleLocale, 否则 LANG (do_status / 主流程都用)
detect_locale() {
    if [[ -n "$opt_locale" ]]; then
        printf '%s' "$opt_locale"
        return
    fi
    if [[ "$(uname)" == "Darwin" ]]; then
        local apple
        apple="$(defaults read -g AppleLocale 2>/dev/null || true)"
        printf '%s' "${apple%%_*}"
        return
    fi
    local lang="${LANG%%.*}"
    [[ "$lang" == "C" || "$lang" == "POSIX" ]] && lang=""
    printf '%s' "$lang"
}

# === --status: 检测当前部署状态 ===
do_status() {
    echo "==> ssh-ops 部署状态"
    echo
    echo "[skill]"
    if [[ -d "$SSHOPS_DEST" ]]; then
        ok "$SSHOPS_DEST/"
        if [[ -f "$SSHOPS_DEST/SKILL.md" ]]; then
            local desc
            desc="$(grep -m1 '^description:' "$SSHOPS_DEST/SKILL.md" | sed 's/^description: *//')"
            inf "  description: ${desc:0:80}…"
        fi
    else
        miss "$SSHOPS_DEST/ (未部署)"
    fi
    if [[ -e "$SSHOPS_OLD_DEST" ]]; then
        warn "  旧 skill 目录残留: $SSHOPS_OLD_DEST (跑 bash install.sh 会自动清理)"
    fi
    echo
    echo "[WezTerm-SSH GUI bundle]"
    if [[ -d "$WEZ_APP" ]]; then
        ok "$WEZ_APP"
        for bin in WezTerm-SSH WezTerm-SSH-cli WezTerm-SSH-mux; do
            local f="$WEZ_APP/Contents/MacOS/$bin"
            if [[ -x "$f" ]]; then
                inf "  $bin ($(stat -f '%z' "$f" 2>/dev/null | numfmt --to=iec 2>/dev/null || stat -f '%z' "$f") bytes)"
            else
                warn "  $bin 缺失"
            fi
        done
    else
        miss "$WEZ_APP (未部署)"
    fi
    echo
    echo "[~/.local/bin wrapper]"
    for cmd in WezTerm-SSH-cli WezTerm-SSH-mux; do
        if [[ -e "$LOCAL_BIN/$cmd" ]]; then
            ok "$LOCAL_BIN/$cmd"
        else
            miss "$LOCAL_BIN/$cmd (未部署)"
        fi
    done
    # 旧命名残留
    for old in wezterm wezterm-gui wezterm-mux-server; do
        if [[ -e "$LOCAL_BIN/$old" ]]; then
            warn "旧命名残留: $LOCAL_BIN/$old (建议 rm 清掉, 与 fork 命名空间冲突)"
        fi
    done
    echo
    echo "[当前运行进程]"
    local gui_pid daemon_pid
    gui_pid="$(/bin/ps -A 2>/dev/null | awk '/WezTerm-SSH.app\/Contents\/MacOS\/WezTerm-SSH/ && !/grep/ {print $1; exit}' || true)"
    daemon_pid="$(pgrep -f sshops-daemon 2>/dev/null | head -1 || true)"
    [[ -n "$gui_pid" ]]    && ok "WezTerm-SSH GUI: pid=$gui_pid"     || miss "WezTerm-SSH GUI 未运行"
    [[ -n "$daemon_pid" ]] && ok "sshops-daemon:   pid=$daemon_pid"  || miss "sshops-daemon 未运行"
    echo
    echo "[ssh-ops 业务 CLI]"
    local rust_bin="$SSHOPS_SRC/rust/target/release/sshops-rs"
    if [[ -x "$rust_bin" ]]; then
        ok "rust binary: $rust_bin"
        inf "  version: $($rust_bin --version 2>&1 | head -1)"
    else
        miss "rust binary 未编译 (cd rust && cargo build --release)"
    fi
    if [[ -f "$SSHOPS_SRC/config.json" ]]; then
        local cli_path
        cli_path="$(jq -r '.wezterm.cli_path // "(unset)"' "$SSHOPS_SRC/config.json" 2>/dev/null || echo "(jq err)")"
        ok "config.json: cli_path=$cli_path"
    else
        miss "config.json 不存在 (跑 sshops setup 创建)"
    fi
    echo
    echo "[~/.zshenv PATH]"
    # ~/.zshenv 里 install.sh 写入的是 $HOME 字面 (跨机器 portable), 不是绝对路径
    for entry in '$HOME/Code/ssh-ops/bin' '$HOME/.local/bin'; do
        if grep -qF "$entry" "$HOME/.zshenv" 2>/dev/null; then
            ok "PATH 含 $entry"
        else
            miss "PATH 不含 $entry (跑 bash install.sh 自动写)"
        fi
    done
    echo
    echo "[locale]"
    inf "  当前系统 locale: $(detect_locale 2>/dev/null || echo unknown)"
    inf "  descriptions.json 含 $(python3 -c "import json; print(len(json.load(open('$SKILL_LOCALES_DIR/descriptions.json'))))" 2>/dev/null || echo '?') 种语言"
}

# === --uninstall: 卸载所有部署, 不动源仓库与录像 ===
do_uninstall() {
    echo "==> 即将卸载 ssh-ops 所有部署 (源仓库与录像不删)"
    echo
    echo "  ✗ $SSHOPS_DEST/"
    echo "  ✗ $SSHOPS_OLD_DEST/ (旧名残留, 如有)"
    echo "  ✗ $WEZ_APP/"
    echo "  ✗ $LOCAL_BIN/{WezTerm-SSH-cli, WezTerm-SSH-mux} wrapper"
    echo "  ✗ ~/.zshenv 里的 PATH 行 (~/Code/ssh-ops/bin 与 ~/.local/bin)"
    echo
    echo "  (保留: 源仓库 $SSHOPS_SRC, 录像数据, 用户 ~/.config/wezterm/, 旧 wezterm-gui 进程)"
    echo
    if (( opt_yes == 0 )); then
        printf '继续? [y/N] '
        read -r ans
        if [[ "$ans" != "y" && "$ans" != "Y" ]]; then
            warn "取消"
            exit 0
        fi
    fi
    echo
    # 1. 停 daemon (有的话)
    local daemon_bin="$SSHOPS_SRC/rust/target/release/sshops-rs"
    if [[ -x "$daemon_bin" ]]; then
        "$daemon_bin" daemon-stop >/dev/null 2>&1 && ok "daemon stopped" || true
    fi
    # 2. 删 skill 目录
    [[ -e "$SSHOPS_DEST" ]]     && rm -rf "$SSHOPS_DEST"     && ok "rm $SSHOPS_DEST"
    [[ -e "$SSHOPS_OLD_DEST" ]] && rm -rf "$SSHOPS_OLD_DEST" && ok "rm $SSHOPS_OLD_DEST"
    # 3. 删 .app bundle
    [[ -e "$WEZ_APP" ]] && rm -rf "$WEZ_APP" && ok "rm $WEZ_APP"
    # 4. 删 wrapper + 旧命名残留
    for f in WezTerm-SSH-cli WezTerm-SSH-mux wezterm wezterm-gui wezterm-mux-server wezterm-gui.real-bin sshops-cli sshops-mux; do
        if [[ -e "$LOCAL_BIN/$f" || -L "$LOCAL_BIN/$f" ]]; then
            rm -f "$LOCAL_BIN/$f"
            ok "rm $LOCAL_BIN/$f"
        fi
    done
    # 5. 清 ~/.zshenv PATH 行
    if [[ -w "$HOME/.zshenv" ]]; then
        local before after
        before="$(wc -l < "$HOME/.zshenv")"
        # 用 grep -v 排除 ssh-ops/bin 与 .local/bin 这两条 ssh-ops 写入的 PATH 行
        # (其他 .local/bin 行可能是用户自己的, 但若是本脚本写的 grep -F 模式精确删)
        local tmp; tmp="$(mktemp)"
        grep -vF 'export PATH="$HOME/Code/ssh-ops/bin:$PATH"' "$HOME/.zshenv" 2>/dev/null \
            | grep -vF 'export PATH="$HOME/.local/bin:$PATH"' > "$tmp" || true
        mv "$tmp" "$HOME/.zshenv"
        after="$(wc -l < "$HOME/.zshenv")"
        ok "~/.zshenv: 清掉 $((before - after)) 行 PATH"
    fi
    # 6. LaunchServices 刷新 (让 macOS Dock / Spotlight 忘掉 WezTerm-SSH.app)
    local lsreg=/System/Library/Frameworks/CoreServices.framework/Versions/A/Frameworks/LaunchServices.framework/Versions/A/Support/lsregister
    [[ -x "$lsreg" ]] && "$lsreg" -kill -domain local -domain user >/dev/null 2>&1 || true
    echo
    ok "卸载完成"
    echo "  源仓库 $SSHOPS_SRC 仍在 (rm -rf 自删 if 需要)"
    echo "  Claude Code 重启 (或新会话) 后 /sshops slash 命令会消失"
    exit 0
}

# --status / --uninstall 跑完即退, 不走安装流程
if (( opt_status == 1 )); then
    do_status
    exit 0
fi
if (( opt_uninstall == 1 )); then
    do_uninstall   # 内部 exit 0
fi

check_cmd() {
    # check_cmd <name> <required> <hint>  (兼容老调用, 仅检测不自动装)
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

# ensure_cmd <name> <required: 0|1> <brew_pkg> <linux_hint>
# macOS 缺则自动 brew install; Linux 只报提示 (apt/yum 用户偏好不一)
ensure_cmd() {
    local cmd="$1" required="$2" brew_pkg="$3" linux_hint="$4"
    if command -v "$cmd" >/dev/null 2>&1; then
        ok "$cmd: $(command -v "$cmd")"
        return 0
    fi
    if [[ "$(uname)" == "Darwin" ]]; then
        if ! command -v brew >/dev/null 2>&1; then
            err "$cmd 缺失, 且 Homebrew 不在 PATH"
            err "  先装 Homebrew: /bin/bash -c \"\$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)\""
            return 1
        fi
        warn "$cmd 缺失, 自动跑: brew install $brew_pkg"
        if brew install $brew_pkg >/dev/null 2>&1; then
            if command -v "$cmd" >/dev/null 2>&1; then
                ok "$cmd 装完: $(command -v "$cmd")"
                return 0
            fi
            warn "brew install $brew_pkg 完成但 $cmd 仍不在 PATH (需新 shell?)"
            return $(( required == 1 ? 1 : 0 ))
        fi
        err "brew install $brew_pkg 失败"
        return $(( required == 1 ? 1 : 0 ))
    fi
    # Linux
    if [[ "$required" == "1" ]]; then
        err "$cmd 缺失。Linux 安装: $linux_hint"
        return 1
    fi
    warn "$cmd 缺失(可选)。Linux 安装: $linux_hint"
    return 0
}

fail=0

if (( opt_link_only == 0 )); then
    echo "==> 检查系统依赖"
    # 自动装 (macOS 缺则 brew install): asciinema / jq / sshpass / pass
    ensure_cmd asciinema 1 "asciinema"                       "pip install asciinema 或 apt install asciinema" || fail=1
    ensure_cmd jq        1 "jq"                              "apt install jq" || fail=1
    ensure_cmd sshpass   0 "hudochenkov/sshpass/sshpass"     "apt install sshpass" || true
    ensure_cmd pass      0 "pass"                            "apt install pass" || true
    # 系统自带, 仅检测
    check_cmd  ssh       1 "(自带,通常无需安装)" || fail=1
    check_cmd  perl      1 "(自带,通常无需安装)" || fail=1
    check_cmd  xxd       1 "(自带,通常无需安装)" || fail=1
    # cargo 走 rustup curl, 不自动跑 (有交互); 缺则报清晰指引
    if ! command -v cargo >/dev/null 2>&1; then
        err "cargo 缺失 (构建 wezterm-src fork 必需)"
        err "  装 rustup: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y"
        err "  装完后: source \$HOME/.cargo/env && bash install.sh"
        fail=1
    else
        ok "cargo: $(command -v cargo)"
    fi

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
echo "==> 检测 locale 并选 description 翻译 (detect_locale 在文件顶部已定义)"

LOCALE="$(detect_locale)"
[[ -z "$LOCALE" ]] && LOCALE="en"
ok "system locale: $LOCALE"

echo
echo "==> 部署 skill 到 $SSHOPS_DEST"

# 旧目录名 ~/.claude/skills/ssh-ops/ (slash 命令是 /ssh-ops) 已重命名为 sshops, 自动清理
if [[ -L "$SSHOPS_OLD_DEST" || -d "$SSHOPS_OLD_DEST" ]]; then
    rm -rf "$SSHOPS_OLD_DEST"
    warn "已清理旧 skill 目录: $SSHOPS_OLD_DEST (slash 命令从 /ssh-ops → /sshops)"
fi

# sshops 目录策略: 实体目录 + symlink, 让 SKILL.md 可独立按 locale 切换
mkdir -p "$SSHOPS_DEST"

# SKILL.md: cp 项目根 (英文主) + python 替换 description 为当前 locale 翻译
# 翻译数据全在 skill-locales/descriptions.json, 31 种语言只维护一个 JSON
DESCRIPTIONS_JSON="$SKILL_LOCALES_DIR/descriptions.json"
cp -f "$SSHOPS_SRC/SKILL.md" "$SSHOPS_DEST/SKILL.md"
python3 - "$LOCALE" "$DESCRIPTIONS_JSON" "$SSHOPS_DEST/SKILL.md" <<'PY'
import json, re, sys
locale, json_path, skill_path = sys.argv[1], sys.argv[2], sys.argv[3]
with open(json_path, encoding="utf-8") as f:
    descs = json.load(f)
# 系统 locale 别名规一
ALIAS = {
    "zh-Hans": "zh-CN", "zh": "zh-CN", "zh_CN": "zh-CN",
    "zh-Hant": "zh-TW", "zh_TW": "zh-TW", "zh-HK": "zh-TW", "zh_HK": "zh-TW",
}
loc = ALIAS.get(locale, locale)
# 精确 → 语言前缀 → en fallback
desc = descs.get(loc) or descs.get(loc.split("-")[0]) or descs["en"]
text = open(skill_path, encoding="utf-8").read()
new_text = re.sub(r"^description:.*$", f"description: {desc}", text, count=1, flags=re.MULTILINE)
open(skill_path, "w", encoding="utf-8").write(new_text)
# 反馈实际选中的 locale (loc 跟 desc 来源)
chosen = loc if loc in descs else (loc.split("-")[0] if loc.split("-")[0] in descs else "en")
print(f"  description ← {chosen}")
PY
ok "SKILL.md 部署完成 (英文正文 + 当前 locale description)"

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
ok "业务文件 (bin/ rust/ ...) 已链接到源仓库"

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

# === --auto-approve: patch Claude Code settings.json 让 sshops 命令免确认 ===
if (( opt_auto_approve == 1 )); then
    echo
    echo "==> --auto-approve: 写 ~/.claude/settings.json (Claude Code sshops 命令免 yes 确认)"
    CLAUDE_CFG="$HOME/.claude/settings.json"
    mkdir -p "$(dirname "$CLAUDE_CFG")"
    [[ -f "$CLAUDE_CFG" ]] || printf '{}\n' > "$CLAUDE_CFG"
    cp "$CLAUDE_CFG" "$CLAUDE_CFG.bak"
    if python3 - "$CLAUDE_CFG" <<'PY'
import json, sys
p = sys.argv[1]
with open(p) as f:
    cfg = json.load(f)
perms = cfg.setdefault('permissions', {})
allow = perms.setdefault('allow', [])
# 两种 pattern 风格都加, 兼容不同 Claude Code 版本
patterns = ['Bash(sshops:*)', 'Bash(sshops *)']
added = [p for p in patterns if p not in allow]
allow.extend(added)
with open(p, 'w') as f:
    json.dump(cfg, f, indent=2)
    f.write('\n')
print('added' if added else 'noop', added)
PY
    then
        ok "settings.json 已 patch (备份: $CLAUDE_CFG.bak)"
        inf "  下次 Claude Code 跑 sshops 命令不再弹 yes 提示"
    else
        warn "settings.json patch 失败 (python3 错误); 已留备份, 请手动加 Bash(sshops:*) 到 permissions.allow"
    fi
fi

echo
echo "==> 下一步"
echo "  1) 启动新 shell (让 ~/.zshenv 的 PATH 生效) 或: source ~/.zshenv"
echo "  2) sshops setup            # 交互式向导写 config.json"
echo "  3) open -a WezTerm-SSH     # 启动 GUI (双击 .app 同效果)"
echo "  4) bash $SSHOPS_SRC/tests/self-test.sh   # 冒烟测试 (需 ssh localhost 可登录)"
echo
echo "Claude Code 中调用:  /sshops <你的请求>  (slash 命令名跟 SKILL frontmatter 一致)"
echo "切换 SKILL 语言:    bash install.sh --link-only --locale en  (默认从系统检测)"
