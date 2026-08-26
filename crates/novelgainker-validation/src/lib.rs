use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt::Write as _;

pub const MAX_FRONTMATTER_BYTES: usize = 32 * 1024;
pub const MAX_FRONTMATTER_LINES: usize = 256;
pub const MAX_FRONTMATTER_LINE_BYTES: usize = 1_024;
pub const MAX_FRONTMATTER_DEPTH: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticSeverity {
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceDiagnostic {
    pub code: &'static str,
    pub severity: DiagnosticSeverity,
    pub relative_path: String,
    pub line: u32,
    pub column: u32,
    pub message: &'static str,
    pub recovery: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationReport {
    pub relative_path: String,
    pub content_hash: String,
    pub diagnostics: Vec<SourceDiagnostic>,
    pub eligible_for_ai_apply: bool,
    pub eligible_for_final: bool,
    pub eligible_for_epub: bool,
}

#[derive(Clone, Copy)]
struct DiagnosticCopy {
    code: &'static str,
    message: &'static str,
    recovery: &'static str,
}

const INVALID_UTF8: DiagnosticCopy = DiagnosticCopy {
    code: "markdown.invalid_utf8",
    message: "文件不是有效的 UTF-8 文本。",
    recovery: "请在外部编辑器中转换为 UTF-8，再重新打开或保存。",
};
const FRONTMATTER_MISSING: DiagnosticCopy = DiagnosticCopy {
    code: "frontmatter.missing",
    message: "源文件缺少 YAML Frontmatter。",
    recovery: "请在文件开头添加由 --- 包围的元数据，并填写 title 与 writingLanguage。",
};
const FRONTMATTER_UNCLOSED: DiagnosticCopy = DiagnosticCopy {
    code: "frontmatter.unclosed",
    message: "YAML Frontmatter 没有结束分隔线。",
    recovery: "请在元数据末尾单独添加一行 ---。",
};
const FRONTMATTER_TOO_LARGE: DiagnosticCopy = DiagnosticCopy {
    code: "frontmatter.too_large",
    message: "YAML Frontmatter 超过安全解析上限。",
    recovery: "请将元数据缩减到 32 KiB、256 行以内，并把正文移到 Frontmatter 之后。",
};
const FRONTMATTER_LINE_TOO_LONG: DiagnosticCopy = DiagnosticCopy {
    code: "frontmatter.line_too_long",
    message: "YAML Frontmatter 中有一行超过安全长度。",
    recovery: "请将该行缩短到 1024 字节以内。",
};
const FRONTMATTER_TOO_DEEP: DiagnosticCopy = DiagnosticCopy {
    code: "frontmatter.too_deep",
    message: "YAML Frontmatter 的嵌套层级超过安全上限。",
    recovery: "请将元数据层级缩减到 8 层以内。",
};
const FRONTMATTER_INVALID_LINE: DiagnosticCopy = DiagnosticCopy {
    code: "frontmatter.invalid_line",
    message: "YAML Frontmatter 中存在当前版本无法安全解析的结构。",
    recovery: "请使用简单的 key: value、缩进映射或列表，移除锚点、标签、流式结构和多行标量。",
};
const FRONTMATTER_DUPLICATE_KEY: DiagnosticCopy = DiagnosticCopy {
    code: "frontmatter.duplicate_key",
    message: "YAML Frontmatter 中存在重复的顶层字段。",
    recovery: "请保留一个字段值并删除重复字段。",
};
const REQUIRED_TITLE: DiagnosticCopy = DiagnosticCopy {
    code: "frontmatter.required_title",
    message: "YAML Frontmatter 缺少非空 title。",
    recovery: "请添加 title: 小说、角色或章节名称。",
};
const REQUIRED_WRITING_LANGUAGE: DiagnosticCopy = DiagnosticCopy {
    code: "frontmatter.required_writing_language",
    message: "YAML Frontmatter 缺少 writingLanguage。",
    recovery: "请添加 writingLanguage: zh-CN。",
};
const INVALID_WRITING_LANGUAGE: DiagnosticCopy = DiagnosticCopy {
    code: "frontmatter.invalid_writing_language",
    message: "writingLanguage 不是当前版本要求的简体中文。",
    recovery: "请将字段修改为 writingLanguage: zh-CN。",
};
const DRAFT_HEADING_MISSING: DiagnosticCopy = DiagnosticCopy {
    code: "chapter.draft_heading_missing",
    message: "章节缺少严格的 ## Draft 标题。",
    recovery: "请在 Frontmatter 后单独添加一行 ## Draft，并把章节正文放在其下。",
};
const DRAFT_HEADING_DUPLICATE: DiagnosticCopy = DiagnosticCopy {
    code: "chapter.draft_heading_duplicate",
    message: "章节包含多个 ## Draft 标题。",
    recovery: "请只保留一个 ## Draft，并合并重复区段的正文。",
};
const DRAFT_EMPTY: DiagnosticCopy = DiagnosticCopy {
    code: "chapter.draft_empty",
    message: "章节的 ## Draft 区段没有正文。",
    recovery: "请在 ## Draft 下填写章节正文。",
};

#[derive(Default)]
struct ParsedFrontmatter {
    top_level: BTreeMap<String, (String, usize)>,
    body_start_line: usize,
}

pub fn validate_source(relative_path: &str, content: &str) -> ValidationReport {
    validate_source_with_hash(relative_path, content, sha256_hex(content.as_bytes()))
}

pub fn validate_source_bytes(relative_path: &str, bytes: &[u8]) -> ValidationReport {
    let content_hash = sha256_hex(bytes);
    let body = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes);
    match std::str::from_utf8(body) {
        Ok(content) => validate_source_with_hash(relative_path, content, content_hash),
        Err(error) => {
            let valid_prefix = &body[..error.valid_up_to()];
            let line = valid_prefix.iter().filter(|byte| **byte == b'\n').count() + 1;
            let column = valid_prefix
                .rsplit(|byte| *byte == b'\n')
                .next()
                .map_or(1, |tail| tail.len() + 1);
            build_report(
                relative_path,
                content_hash,
                vec![diagnostic(relative_path, line, column, INVALID_UTF8)],
            )
        }
    }
}

fn validate_source_with_hash(
    relative_path: &str,
    content: &str,
    content_hash: String,
) -> ValidationReport {
    if relative_path.eq_ignore_ascii_case("AGENTS.md") {
        return build_report(relative_path, content_hash, Vec::new());
    }

    let mut diagnostics = Vec::new();
    let parsed = parse_frontmatter(relative_path, content, &mut diagnostics);
    if let Some(frontmatter) = parsed.as_ref() {
        validate_required_fields(relative_path, frontmatter, &mut diagnostics);
    }
    if is_chapter(relative_path) {
        validate_chapter(
            relative_path,
            content,
            parsed.as_ref().map_or(0, |value| value.body_start_line),
            &mut diagnostics,
        );
    }
    diagnostics.sort_by(|left, right| {
        left.line
            .cmp(&right.line)
            .then_with(|| left.column.cmp(&right.column))
            .then_with(|| left.code.cmp(right.code))
    });
    build_report(relative_path, content_hash, diagnostics)
}

fn parse_frontmatter(
    relative_path: &str,
    content: &str,
    diagnostics: &mut Vec<SourceDiagnostic>,
) -> Option<ParsedFrontmatter> {
    let normalized = content.strip_prefix('\u{feff}').unwrap_or(content);
    let mut lines = normalized.lines();
    if lines.next() != Some("---") {
        diagnostics.push(diagnostic(relative_path, 1, 1, FRONTMATTER_MISSING));
        return None;
    }

    let mut parsed = ParsedFrontmatter::default();
    let mut consumed_bytes = 4_usize;
    let mut closed = false;
    let mut previous_indent = 0_usize;
    for (index, line) in lines.enumerate() {
        let line_number = index + 2;
        consumed_bytes = consumed_bytes.saturating_add(line.len()).saturating_add(1);
        if line == "---" {
            parsed.body_start_line = line_number;
            closed = true;
            break;
        }
        if consumed_bytes > MAX_FRONTMATTER_BYTES || line_number > MAX_FRONTMATTER_LINES + 1 {
            diagnostics.push(diagnostic(
                relative_path,
                line_number,
                1,
                FRONTMATTER_TOO_LARGE,
            ));
            return None;
        }
        if line.len() > MAX_FRONTMATTER_LINE_BYTES {
            diagnostics.push(diagnostic(
                relative_path,
                line_number,
                1,
                FRONTMATTER_LINE_TOO_LONG,
            ));
            continue;
        }
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        if line.contains('\t') || line.chars().any(char::is_control) {
            diagnostics.push(diagnostic(
                relative_path,
                line_number,
                1,
                FRONTMATTER_INVALID_LINE,
            ));
            continue;
        }

        let indent = line.bytes().take_while(|byte| *byte == b' ').count();
        let depth = indent / 2;
        if indent % 2 != 0 || depth > MAX_FRONTMATTER_DEPTH || indent > previous_indent + 2 {
            diagnostics.push(diagnostic(
                relative_path,
                line_number,
                indent + 1,
                if depth > MAX_FRONTMATTER_DEPTH {
                    FRONTMATTER_TOO_DEEP
                } else {
                    FRONTMATTER_INVALID_LINE
                },
            ));
            continue;
        }
        previous_indent = indent;
        let trimmed = &line[indent..];
        if trimmed.starts_with("- ") || trimmed == "-" {
            if indent == 0 || unsafe_yaml_value(trimmed.trim_start_matches('-').trim()) {
                diagnostics.push(diagnostic(
                    relative_path,
                    line_number,
                    indent + 1,
                    FRONTMATTER_INVALID_LINE,
                ));
            }
            continue;
        }

        let Some((key, raw_value)) = trimmed.split_once(':') else {
            diagnostics.push(diagnostic(
                relative_path,
                line_number,
                indent + 1,
                FRONTMATTER_INVALID_LINE,
            ));
            continue;
        };
        if !valid_key(key) || unsafe_yaml_value(raw_value.trim()) {
            diagnostics.push(diagnostic(
                relative_path,
                line_number,
                indent + 1,
                FRONTMATTER_INVALID_LINE,
            ));
            continue;
        }
        let Some(value) = normalized_scalar(raw_value.trim()) else {
            diagnostics.push(diagnostic(
                relative_path,
                line_number,
                indent + key.len() + 2,
                FRONTMATTER_INVALID_LINE,
            ));
            continue;
        };
        if indent == 0 {
            if parsed.top_level.contains_key(key) {
                diagnostics.push(diagnostic(
                    relative_path,
                    line_number,
                    1,
                    FRONTMATTER_DUPLICATE_KEY,
                ));
            } else {
                parsed
                    .top_level
                    .insert(key.to_owned(), (value.to_owned(), line_number));
            }
        }
    }

    if closed {
        Some(parsed)
    } else {
        diagnostics.push(diagnostic(
            relative_path,
            normalized.lines().count().saturating_add(1),
            1,
            FRONTMATTER_UNCLOSED,
        ));
        None
    }
}

fn valid_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 64
        && key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn unsafe_yaml_value(value: &str) -> bool {
    value
        .chars()
        .next()
        .is_some_and(|character| matches!(character, '&' | '*' | '!' | '{' | '[' | '|' | '>'))
        || value.ends_with('}')
        || value.ends_with(']')
        || value.contains(" <<:")
}

fn normalized_scalar(value: &str) -> Option<&str> {
    if value.is_empty() {
        return Some(value);
    }
    if value.starts_with('"') || value.starts_with('\'') {
        let delimiter = value.chars().next()?;
        if value.len() < 2 || !value.ends_with(delimiter) {
            return None;
        }
        Some(&value[1..value.len() - 1])
    } else if value.contains(['"', '\'', '#']) {
        None
    } else {
        Some(value.trim())
    }
}

fn validate_required_fields(
    relative_path: &str,
    parsed: &ParsedFrontmatter,
    diagnostics: &mut Vec<SourceDiagnostic>,
) {
    match parsed.top_level.get("title") {
        Some((value, _)) if !value.trim().is_empty() => {}
        Some((_, line)) => diagnostics.push(diagnostic(relative_path, *line, 1, REQUIRED_TITLE)),
        None => diagnostics.push(diagnostic(relative_path, 2, 1, REQUIRED_TITLE)),
    }
    match parsed.top_level.get("writingLanguage") {
        Some((value, _)) if value == "zh-CN" => {}
        Some((_, line)) => diagnostics.push(diagnostic(
            relative_path,
            *line,
            1,
            INVALID_WRITING_LANGUAGE,
        )),
        None => diagnostics.push(diagnostic(relative_path, 2, 1, REQUIRED_WRITING_LANGUAGE)),
    }
}

fn validate_chapter(
    relative_path: &str,
    content: &str,
    body_start_line: usize,
    diagnostics: &mut Vec<SourceDiagnostic>,
) {
    let lines = content.lines().collect::<Vec<_>>();
    let headings = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| (line.trim_end() == "## Draft").then_some(index))
        .collect::<Vec<_>>();
    if headings.is_empty() {
        diagnostics.push(diagnostic(
            relative_path,
            body_start_line.saturating_add(1).max(1),
            1,
            DRAFT_HEADING_MISSING,
        ));
        return;
    }
    if headings.len() > 1 {
        diagnostics.push(diagnostic(
            relative_path,
            headings[1] + 1,
            1,
            DRAFT_HEADING_DUPLICATE,
        ));
    }
    let first = headings[0];
    let end = lines
        .iter()
        .enumerate()
        .skip(first + 1)
        .find_map(|(index, line)| line.starts_with("## ").then_some(index))
        .unwrap_or(lines.len());
    if !lines[first + 1..end]
        .iter()
        .any(|line| !line.trim().is_empty() && !line.trim_start().starts_with("<!--"))
    {
        diagnostics.push(diagnostic(relative_path, first + 2, 1, DRAFT_EMPTY));
    }
}

fn is_chapter(relative_path: &str) -> bool {
    relative_path
        .split_once('/')
        .is_some_and(|(directory, _)| directory == "chapters")
}

fn diagnostic(
    relative_path: &str,
    line: usize,
    column: usize,
    copy: DiagnosticCopy,
) -> SourceDiagnostic {
    SourceDiagnostic {
        code: copy.code,
        severity: DiagnosticSeverity::Error,
        relative_path: relative_path.to_owned(),
        line: u32::try_from(line.max(1)).unwrap_or(u32::MAX),
        column: u32::try_from(column.max(1)).unwrap_or(u32::MAX),
        message: copy.message,
        recovery: copy.recovery,
    }
}

fn build_report(
    relative_path: &str,
    content_hash: String,
    diagnostics: Vec<SourceDiagnostic>,
) -> ValidationReport {
    let eligible = diagnostics.is_empty();
    ValidationReport {
        relative_path: relative_path.to_owned(),
        content_hash,
        diagnostics,
        eligible_for_ai_apply: eligible,
        eligible_for_final: eligible,
        eligible_for_epub: eligible,
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut value = String::with_capacity(64);
    for byte in digest {
        let _ = write!(&mut value, "{byte:02x}");
    }
    value
}

#[cfg(test)]
mod tests {
    use super::{
        validate_source, validate_source_bytes, ValidationReport, MAX_FRONTMATTER_BYTES,
        MAX_FRONTMATTER_DEPTH,
    };

    fn codes(report: &ValidationReport) -> Vec<&str> {
        report
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code)
            .collect()
    }

    #[test]
    fn valid_chinese_source_and_chapter_golden_cases_are_eligible() {
        let source = "---\ntitle: 人物\nwritingLanguage: zh-CN\n---\n\n正文\n";
        let chapter = "---\ntitle: 第一章\nwritingLanguage: zh-CN\n---\n\n## Draft\n\n这是正文。\n";
        let source_report = validate_source("characters/主角.md", source);
        let chapter_report = validate_source("chapters/001.md", chapter);

        assert!(source_report.diagnostics.is_empty());
        assert!(chapter_report.diagnostics.is_empty());
        assert!(chapter_report.eligible_for_ai_apply);
        assert!(chapter_report.eligible_for_final);
        assert!(chapter_report.eligible_for_epub);
    }

    #[test]
    fn malformed_frontmatter_and_chapter_markup_return_stable_chinese_diagnostics() {
        let report = validate_source(
            "chapters/broken.md",
            "---\ntitle:\nwritingLanguage: en-US\ntitle: 重复\n---\n\n## Draft\n## Draft\n",
        );
        assert!(codes(&report).contains(&"frontmatter.required_title"));
        assert!(codes(&report).contains(&"frontmatter.invalid_writing_language"));
        assert!(codes(&report).contains(&"frontmatter.duplicate_key"));
        assert!(codes(&report).contains(&"chapter.draft_empty"));
        assert!(codes(&report).contains(&"chapter.draft_heading_duplicate"));
        assert!(report
            .diagnostics
            .iter()
            .all(|item| !item.message.is_ascii()));
        assert!(!report.eligible_for_ai_apply);
        assert!(!report.eligible_for_final);
        assert!(!report.eligible_for_epub);
    }

    #[test]
    fn missing_unclosed_and_invalid_utf8_are_bounded_and_never_panic() {
        assert_eq!(
            codes(&validate_source("materials/missing.md", "正文")),
            ["frontmatter.missing"]
        );
        assert_eq!(
            codes(&validate_source(
                "materials/unclosed.md",
                "---\ntitle: 测试"
            )),
            ["frontmatter.unclosed"]
        );
        assert_eq!(
            codes(&validate_source_bytes("materials/bytes.md", &[0xff, 0xfe])),
            ["markdown.invalid_utf8"]
        );
    }

    #[test]
    fn malicious_size_depth_and_yaml_features_stop_at_explicit_limits() {
        let huge = format!("---\ntitle: {}\n---\n", "a".repeat(MAX_FRONTMATTER_BYTES));
        assert!(
            codes(&validate_source("materials/huge.md", &huge)).contains(&"frontmatter.too_large")
        );

        let deep = format!(
            "---\ntitle: 测试\nwritingLanguage: zh-CN\nroot:\n{}leaf: value\n---\n",
            " ".repeat((MAX_FRONTMATTER_DEPTH + 1) * 2)
        );
        assert!(
            codes(&validate_source("materials/deep.md", &deep)).contains(&"frontmatter.too_deep")
        );

        let unsafe_yaml = "---\ntitle: &anchor 测试\nwritingLanguage: zh-CN\n---\n";
        assert!(codes(&validate_source("materials/anchor.md", unsafe_yaml))
            .contains(&"frontmatter.invalid_line"));
    }

    #[test]
    fn agents_convention_file_is_not_treated_as_a_publishable_source() {
        let report = validate_source("AGENTS.md", "# Instructions\n");
        assert!(report.diagnostics.is_empty());
    }
}
