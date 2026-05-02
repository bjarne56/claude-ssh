#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
SecureCRT Keyword List V3 (.ini) -> WezTerm keyword_highlight_rules (Lua) 转换器

用法:
    python3 tools/secure-crt-to-wezterm-rules.py <input.ini> --out lua/wezterm-roy.lua
    或:
    python3 tools/secure-crt-to-wezterm-rules.py <input.ini>   # 输出到 stdout

输入格式(SecureCRT Keyword List V3):
    S:"List Name"=<name>
    D:"Match Case"=<00000000|00000001>
    Z:"Keyword List V3"=<count_hex>
     "<regex>",<BGR_color>,<bold>,<enabled>
     ...

颜色编码:8 位 hex,排列为 00BBGGRR(SecureCRT/Windows GDI 风格,小端 BGR)
转换后输出 #RRGGBB(WezTerm Lua 配置格式)。

输出依赖 fork wezterm 的 keyword_highlight_rules 配置,需先安装 fork 版 WezTerm。
"""
import argparse
import re
import sys
from pathlib import Path


def bgr_to_rgb(bgr_hex: str) -> str:
    """SecureCRT 颜色 8 位 hex(00BBGGRR)→ #RRGGBB"""
    if len(bgr_hex) != 8:
        raise ValueError(f"非法颜色 hex: {bgr_hex!r}")
    bb = bgr_hex[2:4]
    gg = bgr_hex[4:6]
    rr = bgr_hex[6:8]
    return f"#{rr.upper()}{gg.upper()}{bb.upper()}"


def read_ini(path: str) -> str:
    """SecureCRT .ini 可能用 UTF-16-LE(Windows 默认)/ UTF-8 / latin-1。
    先看 BOM,再尝试 UTF-8,最后用 latin-1 兜底。"""
    raw = Path(path).read_bytes()
    if raw.startswith(b"\xff\xfe"):
        return raw[2:].decode("utf-16-le", errors="replace")
    if raw.startswith(b"\xfe\xff"):
        return raw[2:].decode("utf-16-be", errors="replace")
    if raw.startswith(b"\xef\xbb\xbf"):
        return raw[3:].decode("utf-8", errors="replace")
    try:
        return raw.decode("utf-8")
    except UnicodeDecodeError:
        pass
    try:
        return raw.decode("utf-16-le")
    except UnicodeDecodeError:
        pass
    return raw.decode("latin-1", errors="replace")


def parse_ini(text: str):
    """解析 SecureCRT Keyword List V3 .ini。
    返回 (list_name, match_case, rules);rules 元素为 dict
    {regex, color_hex, bold, enabled}。"""
    list_name = ""
    match_case = False
    rules = []
    # 行格式:` "<regex>",<color>,<bold>,<enabled>`
    # 注意:regex 可能含逗号、反斜杠、转义双引号(SecureCRT 极少做转义)
    rule_pattern = re.compile(
        r'^\s*"(.+)",([0-9a-fA-F]{8}),([0-9a-fA-F]{8}),([0-9a-fA-F]{8})\s*$'
    )
    for line in text.splitlines():
        line = line.rstrip()
        if not line:
            continue
        if line.startswith('S:"List Name"='):
            list_name = line.split("=", 1)[1].strip().strip('"')
            continue
        if line.startswith('D:"Match Case"='):
            match_case = line.split("=", 1)[1].strip() == "00000001"
            continue
        m = rule_pattern.match(line)
        if not m:
            continue
        regex_str, color_hex, bold_hex, enabled_hex = m.groups()
        rules.append(
            {
                "regex": regex_str,
                "color_hex": color_hex,
                "bold": bold_hex == "00000001",
                "enabled": enabled_hex == "00000001",
            }
        )
    return list_name, match_case, rules


def lua_string_literal(s: str) -> str:
    """生成 Lua 字符串字面量(单引号 + 反斜杠转义),适合作 regex 字面量。"""
    s = s.replace("\\", "\\\\")
    s = s.replace("'", "\\'")
    return f"'{s}'"


def emit_lua(list_name: str, match_case: bool, rules: list, source_path: str) -> str:
    out = []
    out.append("-- ============================================================")
    out.append("-- 由 tools/secure-crt-to-wezterm-rules.py 自动生成")
    out.append(f"-- 来源:{source_path}")
    out.append(f"-- 列表名称:{list_name}")
    out.append(f"-- Match Case(大小写敏感):{match_case}")
    out.append(f"-- 启用规则数:{sum(1 for r in rules if r['enabled'])}")
    out.append(f"-- 总规则数:{len(rules)}")
    out.append("-- ============================================================")
    out.append("--")
    out.append("-- 用法(在 ~/.wezterm.lua 中):")
    out.append("--")
    out.append("--   local wezterm = require 'wezterm'")
    out.append("--   local config = wezterm.config_builder()")
    out.append("--   config.keyword_highlight_rules = require 'wezterm-roy'")
    out.append("--   return config")
    out.append("--")
    out.append("-- 该文件需要 fork 版 WezTerm(支持 keyword_highlight_rules)。")
    out.append("-- 标准 WezTerm 不识别此配置,会在加载时报错。")
    out.append("-- ============================================================")
    out.append("")
    out.append("local M = {}")
    out.append("")

    if not match_case:
        out.append("-- 原 SecureCRT 配置 Match Case=false,在每条 regex 前加")
        out.append("-- (?i) 内联标志启用大小写不敏感匹配。fancy_regex 支持该语法。")
        out.append("")

    case_prefix = "" if match_case else "(?i)"

    for i, rule in enumerate(rules, 1):
        if not rule["enabled"]:
            out.append(f"-- 规则 {i} 在 SecureCRT 中标记为 disabled,跳过")
            out.append("")
            continue
        try:
            rgb = bgr_to_rgb(rule["color_hex"])
        except ValueError as e:
            out.append(f"-- 规则 {i} 颜色解析失败:{e}")
            continue
        regex_with_flags = f"{case_prefix}{rule['regex']}"
        # 短预览注释,辅助阅读
        preview = rule["regex"]
        if len(preview) > 70:
            preview = preview[:67] + "..."
        out.append(f"-- 规则 {i}: {preview}")
        out.append("table.insert(M, {")
        out.append(f"    regex = {lua_string_literal(regex_with_flags)},")
        out.append(f"    fg = '{rgb}',")
        if rule["bold"]:
            out.append("    bold = true,")
        out.append("})")
        out.append("")

    out.append("-- ============================================================")
    out.append("-- L2 补充候选规则(默认注释,按需启用)")
    out.append("-- 来源:zsh-syntax-highlighting + 常见输出场景盲区")
    out.append("-- ============================================================")
    out.append("")
    out.append("-- L2-1 双引号字符串(JSON / 配置文件常见)")
    out.append("-- table.insert(M, { regex = [[\"[^\"\\\\n]*\"]], fg = '#FFD700' })")
    out.append("")
    out.append("-- L2-2 单引号字符串")
    out.append("-- table.insert(M, { regex = [[\\'[^\\'\\\\n]*\\']], fg = '#FFD700' })")
    out.append("")
    out.append("-- L2-3 shell 变量($PATH / ${HOME})")
    out.append("-- table.insert(M, { regex = [[\\$\\{?\\w+\\}?]], fg = '#C586C0' })")
    out.append("")
    out.append("-- L2-5 git / docker hash(7-40 位 hex)")
    out.append("-- table.insert(M, { regex = [[\\b[0-9a-f]{7,40}\\b]], fg = '#888888' })")
    out.append("")
    out.append("-- L2-6 文件路径(带扩展名)")
    out.append("-- table.insert(M, {")
    out.append("--     regex = [[[\\w./-]+\\.(log|conf|ya?ml|json|sh|py|rs|go|md)\\b]],")
    out.append("--     fg = '#00FFFF',")
    out.append("-- })")
    out.append("")
    out.append("return M")
    out.append("")

    return "\n".join(out)


def main():
    parser = argparse.ArgumentParser(
        description="SecureCRT Keyword List V3 -> WezTerm keyword_highlight_rules"
    )
    parser.add_argument("input", help="SecureCRT 关键字列表 .ini 路径")
    parser.add_argument(
        "--out",
        help="输出 Lua 文件路径(默认输出到 stdout)",
        default=None,
    )
    args = parser.parse_args()

    text = read_ini(args.input)
    list_name, match_case, rules = parse_ini(text)

    if not rules:
        print(
            "warning: 未解析到任何规则,请检查输入文件格式",
            file=sys.stderr,
        )

    lua = emit_lua(list_name, match_case, rules, args.input)

    if args.out:
        Path(args.out).write_text(lua, encoding="utf-8")
        enabled_count = sum(1 for r in rules if r["enabled"])
        print(
            f"已生成 {args.out}(启用 {enabled_count} 条规则,共 {len(rules)} 条)",
            file=sys.stderr,
        )
    else:
        sys.stdout.write(lua)


if __name__ == "__main__":
    main()
