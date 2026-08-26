use crate::project::ProjectHandle;
use crate::workspace::{list_project_files, read_project_file, ProjectFileKind, ProjectFileNode};
use crate::AppError;
use serde::Serialize;

pub const MAX_SEARCH_QUERY_CHARS: usize = 128;
pub const MAX_SEARCH_FILES: usize = 500;
pub const MAX_SEARCH_BYTES: u64 = 32 * 1024 * 1024;
pub const MAX_SEARCH_RESULTS: usize = 100;
pub const MAX_SEARCH_SNIPPET_CHARS: usize = 160;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchMatch {
    pub relative_path: String,
    pub line: u32,
    pub column: u32,
    pub snippet: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchReport {
    pub matches: Vec<SearchMatch>,
    pub scanned_file_count: u32,
    pub skipped_file_count: u32,
    pub scanned_bytes: u64,
    pub truncated: bool,
}

pub fn search_project_sources(
    handle: &ProjectHandle,
    query: &str,
    case_sensitive: bool,
) -> Result<SearchReport, AppError> {
    validate_query(query)?;
    let index = list_project_files(handle)?;
    let mut sources = Vec::new();
    collect_markdown_sources(&index.nodes, &mut sources);

    let mut report = SearchReport {
        matches: Vec::new(),
        scanned_file_count: 0,
        skipped_file_count: 0,
        scanned_bytes: 0,
        truncated: false,
    };
    for node in sources {
        if usize::try_from(report.scanned_file_count).unwrap_or(usize::MAX) >= MAX_SEARCH_FILES {
            report.truncated = true;
            break;
        }
        let size_bytes = node.size_bytes.unwrap_or(0);
        if report.scanned_bytes.saturating_add(size_bytes) > MAX_SEARCH_BYTES {
            report.truncated = true;
            break;
        }
        report.scanned_file_count = report.scanned_file_count.saturating_add(1);
        report.scanned_bytes = report.scanned_bytes.saturating_add(size_bytes);
        let document = match read_project_file(handle, &node.relative_path) {
            Ok(value) => value,
            Err(error)
                if matches!(
                    error.code.as_str(),
                    "file.unsupported_encoding" | "file.too_large" | "file.not_found"
                ) =>
            {
                report.skipped_file_count = report.skipped_file_count.saturating_add(1);
                continue;
            }
            Err(error) => return Err(error),
        };

        for (line_index, line) in document.content.lines().enumerate() {
            for byte_index in literal_match_indices(line, query, case_sensitive) {
                let column = line[..byte_index].chars().count() + 1;
                report.matches.push(SearchMatch {
                    relative_path: document.relative_path.clone(),
                    line: u32::try_from(line_index + 1).unwrap_or(u32::MAX),
                    column: u32::try_from(column).unwrap_or(u32::MAX),
                    snippet: bounded_snippet(line, column - 1),
                });
                if report.matches.len() >= MAX_SEARCH_RESULTS {
                    report.truncated = true;
                    return Ok(report);
                }
            }
        }
    }
    Ok(report)
}

fn validate_query(query: &str) -> Result<(), AppError> {
    let character_count = query.chars().count();
    if character_count == 0
        || character_count > MAX_SEARCH_QUERY_CHARS
        || query.chars().any(char::is_control)
    {
        Err(AppError::search_invalid_query())
    } else {
        Ok(())
    }
}

fn collect_markdown_sources<'a>(
    nodes: &'a [ProjectFileNode],
    output: &mut Vec<&'a ProjectFileNode>,
) {
    for node in nodes {
        match node.kind {
            ProjectFileKind::Directory => collect_markdown_sources(&node.children, output),
            ProjectFileKind::Markdown => output.push(node),
            ProjectFileKind::Metadata => {}
        }
    }
}

fn literal_match_indices(line: &str, query: &str, case_sensitive: bool) -> Vec<usize> {
    if case_sensitive {
        return line.match_indices(query).map(|(index, _)| index).collect();
    }
    line.char_indices()
        .filter_map(|(start, _)| {
            let end = start.checked_add(query.len())?;
            let candidate = line.get(start..end)?;
            candidate.eq_ignore_ascii_case(query).then_some(start)
        })
        .collect()
}

fn bounded_snippet(line: &str, match_character_index: usize) -> String {
    let characters = line
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect::<Vec<_>>();
    let start = match_character_index.saturating_sub(40);
    let end = characters
        .len()
        .min(start.saturating_add(MAX_SEARCH_SNIPPET_CHARS));
    let mut snippet = String::new();
    if start > 0 {
        snippet.push('…');
    }
    snippet.extend(characters[start..end].iter());
    if end < characters.len() {
        snippet.push('…');
    }
    snippet
}

#[cfg(test)]
mod tests {
    use super::{
        search_project_sources, MAX_SEARCH_QUERY_CHARS, MAX_SEARCH_RESULTS,
        MAX_SEARCH_SNIPPET_CHARS,
    };
    use crate::project::{create_project, inspect_project_root, ProjectHandle, ProjectScanMode};
    use std::fs;
    use std::path::PathBuf;
    use uuid::Uuid;

    struct TestProject {
        root: PathBuf,
        handle: ProjectHandle,
    }

    impl TestProject {
        fn new() -> Self {
            let root =
                std::env::temp_dir().join(format!("novelgainker-ng105-search-{}", Uuid::new_v4()));
            fs::create_dir(&root).expect("test root must create");
            let inspection = inspect_project_root(&root, ProjectScanMode::Create)
                .expect("test project must inspect");
            let handle = create_project(&inspection).expect("test project must create");
            Self { root, handle }
        }
    }

    impl Drop for TestProject {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn chinese_and_english_literal_search_is_deterministic_and_navigation_only() {
        let project = TestProject::new();
        let source = "---\ntitle: 第一章\nwritingLanguage: zh-CN\n---\n## Draft\nDeepSeek 看见月光。\ndeepseek 再次出现。\n";
        fs::write(project.root.join("chapters/001.md"), source).unwrap();
        let before = fs::read(project.root.join("chapters/001.md")).unwrap();

        let insensitive =
            search_project_sources(&project.handle, "deepseek", false).expect("search must work");
        let sensitive =
            search_project_sources(&project.handle, "DeepSeek", true).expect("search must work");

        assert_eq!(insensitive.matches.len(), 2);
        assert_eq!(sensitive.matches.len(), 1);
        assert_eq!(insensitive.matches[0].relative_path, "chapters/001.md");
        assert_eq!(insensitive.matches[0].line, 6);
        assert_eq!(insensitive.matches[0].column, 1);
        assert_eq!(
            fs::read(project.root.join("chapters/001.md")).unwrap(),
            before
        );
    }

    #[test]
    fn every_call_rebuilds_from_canon_and_excludes_hidden_published_and_metadata() {
        let project = TestProject::new();
        fs::write(project.root.join("materials/visible.md"), "唯一词").unwrap();
        fs::write(project.root.join("materials/invalid.md"), [0xff, 0xfe]).unwrap();
        fs::create_dir_all(project.root.join("published")).unwrap();
        fs::write(project.root.join("published/hidden.md"), "唯一词").unwrap();
        fs::create_dir_all(project.root.join(".secret")).unwrap();
        fs::write(project.root.join(".secret/key.md"), "唯一词").unwrap();
        fs::write(
            project.root.join(".novelgainker/search-index.json"),
            "{broken",
        )
        .unwrap();

        let first = search_project_sources(&project.handle, "唯一词", false).unwrap();
        assert_eq!(first.matches.len(), 1);
        assert_eq!(first.matches[0].relative_path, "materials/visible.md");
        assert_eq!(first.skipped_file_count, 1);
        assert!(first.scanned_bytes >= 2);
        fs::remove_file(project.root.join(".novelgainker/search-index.json")).unwrap();
        let without_index = search_project_sources(&project.handle, "唯一词", false).unwrap();
        assert_eq!(without_index.matches.len(), 1);

        fs::write(project.root.join("materials/visible.md"), "已修改").unwrap();
        let rebuilt = search_project_sources(&project.handle, "唯一词", false).unwrap();
        assert!(rebuilt.matches.is_empty());
    }

    #[test]
    fn query_results_and_snippets_are_strictly_bounded() {
        let project = TestProject::new();
        let repeated = format!("{}\n", "命中 ".repeat(MAX_SEARCH_RESULTS + 10));
        fs::write(project.root.join("materials/many.md"), repeated).unwrap();
        let report = search_project_sources(&project.handle, "命中", false).unwrap();
        assert_eq!(report.matches.len(), MAX_SEARCH_RESULTS);
        assert!(report.truncated);
        assert!(report
            .matches
            .iter()
            .all(|item| item.snippet.chars().count() <= MAX_SEARCH_SNIPPET_CHARS + 2));

        let error = search_project_sources(
            &project.handle,
            &"q".repeat(MAX_SEARCH_QUERY_CHARS + 1),
            false,
        )
        .expect_err("oversized query must fail");
        assert_eq!(error.code, "search.invalid_query");
        assert_eq!(error.diagnostic, None);
    }
}
