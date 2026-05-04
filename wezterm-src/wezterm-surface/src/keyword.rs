//! 关键字高亮规则:对终端输出文本按正则匹配后改写 cell 颜色属性。
//!
//! 灵感来自 SecureCRT 的 Keyword Highlighting 功能。与 `hyperlink::Rule` 不同:
//! `hyperlink::Rule` 把匹配到的文本变成可点击的 URL;
//! `KeywordHighlightRule` 仅修改前景/背景/字体强调属性,**不**生成超链接。
//!
//! 本模块提供:
//! - `KeywordHighlightRule`:声明式规则结构,字段对应 fg/bg/bold/italic/underline/override_ansi
//! - `KeywordMatch`:一次匹配的结果(字节范围 + 命中的规则索引)
//! - `KeywordHighlightRule::match_keywords`:对单行文本应用一组规则,返回排序后的匹配列表
//!
//! 实际把匹配结果写回 cell 属性的逻辑在 `line::apply_keyword_highlight_rules`(Step 4)。

use core::ops::Range;
use fancy_regex::Regex;
#[cfg(feature = "use_serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use wezterm_cell::color::ColorAttribute;
use wezterm_color_types::SrgbaTuple;
use wezterm_dynamic::{FromDynamic, FromDynamicOptions, ToDynamic, Value};

extern crate alloc;
use crate::alloc::string::ToString;
#[cfg(feature = "use_serde")]
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

/// 单条关键字高亮规则。
///
/// 当 `regex` 在终端输出文本中匹配到子串时,匹配范围内的 cell 会被重写颜色/强调属性。
/// 字段为 `Option`/`bool` 的目的是:
/// - `None`/`false` 表示**不修改**该属性,保留原始(通常来自服务端 ANSI 转义序列)
/// - `Some(_)`/`true` 表示**写入**该属性
///
/// 渲染层最终行为还要看 `override_ansi`:
/// - `override_ansi=false`(默认):仅当 cell 当前 fg/bg 为 default 时写入,服务端 ANSI 颜色保留优先
/// - `override_ansi=true`:无条件覆盖
#[cfg_attr(feature = "use_serde", derive(Deserialize, Serialize))]
#[derive(Debug, Clone, FromDynamic, ToDynamic)]
pub struct KeywordHighlightRule {
    /// 编译后的正则。Lua 配置侧传入字符串,经 `RegexWrap` 自动编译。
    #[cfg_attr(
        feature = "use_serde",
        serde(
            deserialize_with = "deserialize_regex",
            serialize_with = "serialize_regex"
        )
    )]
    #[dynamic(into = "RegexWrap", try_from = "RegexWrap")]
    pub regex: Regex,

    /// 前景色。`None` 表示不修改。Lua 配置侧接受 hex 字符串(`'#FF0000'`)
    /// 或 CSS 颜色名(`'red'`),通过 SrgbaTuple::FromDynamic 反序列化。
    #[dynamic(default)]
    pub fg: Option<SrgbaTuple>,

    /// 背景色。`None` 表示不修改。同 `fg`,接受 hex / CSS 颜色字符串。
    #[dynamic(default)]
    pub bg: Option<SrgbaTuple>,

    /// 是否将匹配文本设为粗体。
    #[dynamic(default)]
    pub bold: bool,

    /// 是否将匹配文本设为斜体。
    #[dynamic(default)]
    pub italic: bool,

    /// 是否给匹配文本加下划线。
    #[dynamic(default)]
    pub underline: bool,

    /// 是否覆盖 cell 已有的颜色(典型来源:服务端发送的 ANSI 转义序列)。
    /// 默认 `false`:服务端 ANSI 优先,本规则只在 cell 颜色为 default 时生效;
    /// 设为 `true`:无条件覆盖。
    #[dynamic(default)]
    pub override_ansi: bool,
}

/// 一次成功匹配的描述。渲染层(Step 4 中的 `apply_keyword_highlight_rules`)
/// 用 `range` 定位 cell,用 `rule_index` 反查 fg/bg/bold/... 写回 cell 属性。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeywordMatch {
    /// 匹配文本在输入字符串中的字节范围(并非 cell 索引;cell 转换在渲染层完成)。
    pub range: Range<usize>,
    /// 命中的规则在 `rules` 切片中的索引。
    pub rule_index: usize,
}

/// 围绕 `Regex` 的薄包装,用于 `wezterm-dynamic` 反序列化时把 Lua 字符串编译成 `Regex`。
struct RegexWrap(Regex);

impl FromDynamic for RegexWrap {
    fn from_dynamic(
        value: &Value,
        options: FromDynamicOptions,
    ) -> Result<RegexWrap, wezterm_dynamic::Error> {
        let s = String::from_dynamic(value, options)?;
        Ok(RegexWrap(Regex::new(&s).map_err(|e| e.to_string())?))
    }
}

impl From<&Regex> for RegexWrap {
    fn from(regex: &Regex) -> RegexWrap {
        RegexWrap(regex.clone())
    }
}

impl Into<Regex> for RegexWrap {
    fn into(self) -> Regex {
        self.0
    }
}

impl ToDynamic for RegexWrap {
    fn to_dynamic(&self) -> Value {
        self.0.to_string().to_dynamic()
    }
}

#[cfg(feature = "use_serde")]
fn deserialize_regex<'de, D>(deserializer: D) -> Result<Regex, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Regex::new(&s).map_err(|e| serde::de::Error::custom(format!("{:?}", e)))
}

#[cfg(feature = "use_serde")]
fn serialize_regex<S>(regex: &Regex, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let s = regex.to_string();
    s.serialize(serializer)
}

impl KeywordHighlightRule {
    /// 用一个正则字符串构造一条空属性规则。失败原因仅可能是正则编译错误。
    pub fn new(regex: &str) -> Result<Self, fancy_regex::Error> {
        Ok(Self {
            regex: Regex::new(regex)?,
            fg: None,
            bg: None,
            bold: false,
            italic: false,
            underline: false,
            override_ansi: false,
        })
    }

    pub fn with_fg(mut self, fg: SrgbaTuple) -> Self {
        self.fg = Some(fg);
        self
    }

    pub fn with_bg(mut self, bg: SrgbaTuple) -> Self {
        self.bg = Some(bg);
        self
    }

    pub fn with_bold(mut self, bold: bool) -> Self {
        self.bold = bold;
        self
    }

    pub fn with_italic(mut self, italic: bool) -> Self {
        self.italic = italic;
        self
    }

    pub fn with_underline(mut self, underline: bool) -> Self {
        self.underline = underline;
        self
    }

    pub fn with_override_ansi(mut self, override_ansi: bool) -> Self {
        self.override_ansi = override_ansi;
        self
    }

    /// 把一组规则应用到一行文本,返回所有匹配,按**长度降序**排序。
    ///
    /// 长度降序的目的:多条规则在同一片文本上重叠时,渲染层会按返回顺序写入 cell,
    /// 后写的更短匹配可能覆盖先写的更长匹配。这里先排好序,把"更具体"的匹配放前面,
    /// 让短匹配能在最后落到具体的子段上(与 hyperlink::Rule::match_hyperlinks 同款策略)。
    pub fn match_keywords(line: &str, rules: &[KeywordHighlightRule]) -> Vec<KeywordMatch> {
        let mut entries: Vec<(usize, Range<usize>, usize)> = Vec::new();

        for (rule_index, rule) in rules.iter().enumerate() {
            for capture_result in rule.regex.captures_iter(line) {
                if let Ok(captures) = capture_result {
                    if let Some(m) = captures.get(0) {
                        let range = m.start()..m.end();
                        let len = range.end.saturating_sub(range.start);
                        if len > 0 {
                            entries.push((len, range, rule_index));
                        }
                    }
                }
            }
        }

        // 长匹配在前。同长度则保持发现顺序(稳定排序)。
        entries.sort_by(|a, b| b.0.cmp(&a.0));

        entries
            .into_iter()
            .map(|(_, range, rule_index)| KeywordMatch { range, rule_index })
            .collect()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn red() -> SrgbaTuple {
        SrgbaTuple(1.0, 0.0, 0.0, 1.0)
    }

    fn green() -> SrgbaTuple {
        SrgbaTuple(0.0, 1.0, 0.0, 1.0)
    }

    #[test]
    fn match_basic_keywords() {
        // 基础双规则:故障词标红,健康词标绿
        let rules = vec![
            KeywordHighlightRule::new(r"\b(error|fail|down)\b")
                .unwrap()
                .with_fg(red()),
            KeywordHighlightRule::new(r"\b(up|active|running)\b")
                .unwrap()
                .with_fg(green()),
        ];

        let line = "service is running, db connection is down: error code";
        let matches = KeywordHighlightRule::match_keywords(line, &rules);

        // 期望命中三处:running / down / error
        assert_eq!(matches.len(), 3);

        // 验证每条匹配 range 与 rule_index 落在合法区间
        for m in &matches {
            assert!(m.range.start < m.range.end, "range 必须非空: {:?}", m.range);
            assert!(m.range.end <= line.len(), "range 不应越界");
            assert!(m.rule_index < rules.len(), "rule_index 越界");
        }
    }

    #[test]
    fn match_sorted_by_length_desc() {
        // 两条规则可能匹配同一段文本时,长匹配应排在前
        let rules = vec![
            KeywordHighlightRule::new(r"\b(established)\b").unwrap(),
            KeywordHighlightRule::new(r"\b(est)\w*\b").unwrap(),
        ];

        let matches = KeywordHighlightRule::match_keywords("connection established", &rules);
        assert_eq!(matches.len(), 2);
        // 第一条长度 == 第二条长度("established" 全词都被两条规则匹配)
        assert!(matches[0].range.end - matches[0].range.start
            >= matches[1].range.end - matches[1].range.start);
    }

    #[test]
    fn override_ansi_flag_default_and_setter() {
        // 默认 override_ansi 为 false
        let rule = KeywordHighlightRule::new(r"foo").unwrap();
        assert!(!rule.override_ansi);

        // setter 能开
        let rule2 = KeywordHighlightRule::new(r"foo")
            .unwrap()
            .with_override_ansi(true);
        assert!(rule2.override_ansi);
    }

    #[test]
    fn bold_italic_underline_setters() {
        let rule = KeywordHighlightRule::new(r"x")
            .unwrap()
            .with_bold(true)
            .with_italic(true)
            .with_underline(true);
        assert!(rule.bold);
        assert!(rule.italic);
        assert!(rule.underline);
    }

    #[test]
    fn match_chinese_keywords() {
        // 验证中文/UTF-8 字面量匹配:返回的 range 是字节范围,不应在多字节字符内部切断
        let rules = vec![
            KeywordHighlightRule::new(r"(故障|错误)")
                .unwrap()
                .with_fg(red()),
        ];

        let line = "系统出现故障,日志包含错误信息";
        let matches = KeywordHighlightRule::match_keywords(line, &rules);
        assert_eq!(matches.len(), 2);

        // 切片必须落在合法字符边界(用 utf8 char_indices 验证)
        for m in &matches {
            let slice = &line.as_bytes()[m.range.start..m.range.end];
            // 能成功 from_utf8 说明边界合法
            assert!(
                core::str::from_utf8(slice).is_ok(),
                "range 切片应是合法 UTF-8: {:?}",
                m.range
            );
        }
    }

    #[test]
    fn no_match_returns_empty() {
        let rules = vec![KeywordHighlightRule::new(r"\bnomatch\b")
            .unwrap()
            .with_fg(red())];
        let matches = KeywordHighlightRule::match_keywords("hello world", &rules);
        assert!(matches.is_empty());
    }

    #[test]
    fn empty_rules_returns_empty() {
        let matches = KeywordHighlightRule::match_keywords("anything", &[]);
        assert!(matches.is_empty());
    }

    // ---- Line 集成测试:验证 scan_and_apply_keyword_highlight 把
    // ---- KeywordHighlightRule 实际写到 cell.attrs() 上 ----

    #[cfg(feature = "std")]
    mod line_integration {
        use super::*;
        use crate::line::Line;
        use wezterm_cell::CellAttributes;
        use wezterm_cell::{color::AnsiColor, Cell};

        // 把 SrgbaTuple 包成 ColorAttribute,用于断言 cell.attrs().foreground()
        fn red_attr() -> ColorAttribute {
            ColorAttribute::TrueColorWithDefaultFallback(red())
        }

        fn green_attr() -> ColorAttribute {
            ColorAttribute::TrueColorWithDefaultFallback(green())
        }

        #[test]
        fn cell_fg_rewrite_basic() {
            let mut line: Line = "error: connection refused".into();
            let rules = vec![KeywordHighlightRule::new(r"\berror\b")
                .unwrap()
                .with_fg(red())];

            line.scan_and_apply_keyword_highlight(&rules);

            let cells = line.coerce_vec_storage().to_vec();
            let red = red_attr();
            // "error" 占 cell 0..5
            for i in 0..5 {
                assert_eq!(
                    cells[i].attrs().foreground(),
                    red,
                    "cell {} ({:?}) 应为红色",
                    i,
                    cells[i].str()
                );
            }
            // cell 5 (':') 不被规则匹配,保持 Default
            assert_eq!(cells[5].attrs().foreground(), ColorAttribute::Default);
        }

        #[test]
        fn override_ansi_false_preserves_existing_color() {
            // 构造 cell 已有蓝色 fg(模拟服务端 ANSI)
            let mut line = Line::with_width(5, crate::SEQ_ZERO);
            let blue_attrs = CellAttributes::default()
                .set_foreground(AnsiColor::Blue)
                .clone();
            for (i, ch) in "error".chars().enumerate() {
                line.set_cell(i, Cell::new(ch, blue_attrs.clone()), crate::SEQ_ZERO);
            }

            // 默认 override_ansi=false 不应覆盖
            let rules = vec![KeywordHighlightRule::new(r"\berror\b")
                .unwrap()
                .with_fg(red())];
            line.scan_and_apply_keyword_highlight(&rules);

            let cells = line.coerce_vec_storage().to_vec();
            let blue: ColorAttribute = AnsiColor::Blue.into();
            for i in 0..5 {
                assert_eq!(
                    cells[i].attrs().foreground(),
                    blue,
                    "override_ansi=false 应保留服务端蓝色,cell {}",
                    i
                );
            }
        }

        #[test]
        fn override_ansi_true_forces_overwrite() {
            let mut line = Line::with_width(5, crate::SEQ_ZERO);
            let blue_attrs = CellAttributes::default()
                .set_foreground(AnsiColor::Blue)
                .clone();
            for (i, ch) in "error".chars().enumerate() {
                line.set_cell(i, Cell::new(ch, blue_attrs.clone()), crate::SEQ_ZERO);
            }

            let rules = vec![KeywordHighlightRule::new(r"\berror\b")
                .unwrap()
                .with_fg(red())
                .with_override_ansi(true)];
            line.scan_and_apply_keyword_highlight(&rules);

            let cells = line.coerce_vec_storage().to_vec();
            let red = red_attr();
            for i in 0..5 {
                assert_eq!(
                    cells[i].attrs().foreground(),
                    red,
                    "override_ansi=true 应覆盖蓝色,cell {} 应为红",
                    i
                );
            }
        }

        #[test]
        fn chinese_byte_to_cell_alignment() {
            // UTF-8 中文 3 字节 + 终端双宽字符:验证 byte→cell 转换不错位
            let mut line: Line = "状态: 故障".into();
            let rules = vec![KeywordHighlightRule::new(r"故障")
                .unwrap()
                .with_fg(red())];

            line.scan_and_apply_keyword_highlight(&rules);

            let cells = line.coerce_vec_storage().to_vec();
            let red = red_attr();
            let red_count = cells
                .iter()
                .filter(|c| c.attrs().foreground() == red)
                .count();
            // "故障" 主 cell 各 1 个,加上各自的 spacer(填充蓝色与否取决于实现)
            // 至少有 2 个 cell 被染红
            assert!(
                red_count >= 2,
                "至少 2 个 cell 应被染红,实际 {}",
                red_count
            );
        }

        #[test]
        fn idempotent_via_scanned_bit() {
            // 第二次 scan 在 SCANNED bit 已置位时为 no-op:
            // 用"先 scan、再改 rule、再不 invalidate scan、看 cell 是否被改"验证
            let mut line: Line = "error here".into();
            let red_rule = vec![KeywordHighlightRule::new(r"\berror\b")
                .unwrap()
                .with_fg(red())];
            let green_rule = vec![KeywordHighlightRule::new(r"\berror\b")
                .unwrap()
                .with_fg(green())
                .with_override_ansi(true)];

            line.scan_and_apply_keyword_highlight(&red_rule);
            // 不 invalidate,直接换规则再 scan
            line.scan_and_apply_keyword_highlight(&green_rule);

            // 因为 SCANNED bit 已置位,第二次为 no-op,cell 仍是红色
            let cells = line.coerce_vec_storage().to_vec();
            let red = red_attr();
            assert_eq!(
                cells[0].attrs().foreground(),
                red,
                "未 invalidate 时第二次 scan 应为 no-op,cell 0 仍应是红色"
            );

            // invalidate 后再 scan,应换成绿色
            line.invalidate_keyword_highlight(crate::SEQ_ZERO);
            line.scan_and_apply_keyword_highlight(&green_rule);
            let cells = line.coerce_vec_storage().to_vec();
            let green = green_attr();
            assert_eq!(
                cells[0].attrs().foreground(),
                green,
                "invalidate 后再 scan 应应用新规则,cell 0 应是绿色"
            );
        }
    }
}
