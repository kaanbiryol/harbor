use std::{ops::Range, path::Path, time::Duration};

use gpui::HighlightStyle;
use gpui_component::{
    highlighter::{HighlightTheme, LanguageRegistry, SyntaxHighlighter},
    input::Rope,
};
use harbor_domain::DiffFile;

pub use harbor_domain::diff::DiffLineKind;

const MAX_SYNTAX_PATCH_BYTES: usize = 512 * 1024;
const MAX_SYNTAX_LINE_BYTES: usize = 10_000;
const SYNTAX_PARSE_TIMEOUT: Duration = Duration::from_millis(50);

pub type ParsedDiff = harbor_domain::diff::ParsedDiff<HighlightStyle>;
#[cfg(test)]
pub type DiffHunk = harbor_domain::diff::DiffHunk<HighlightStyle>;
pub type DiffLine = harbor_domain::diff::DiffLine<HighlightStyle>;

pub fn parse_files(files: &[DiffFile]) -> Vec<Option<ParsedDiff>> {
    files
        .iter()
        .map(|file| file.patch.as_deref().map(parse_unified_diff))
        .collect()
}

pub fn parse_files_with_syntax(
    files: &[DiffFile],
    highlight_theme: &HighlightTheme,
) -> Vec<Option<ParsedDiff>> {
    files
        .iter()
        .map(|file| {
            file.patch
                .as_deref()
                .map(|patch| parse_unified_diff_with_syntax(file, patch, highlight_theme))
        })
        .collect()
}

pub fn parse_unified_diff(patch: &str) -> ParsedDiff {
    harbor_domain::diff::parse_unified_diff_with_syntax_payload(patch)
}

pub fn parse_unified_diff_with_syntax(
    file: &DiffFile,
    patch: &str,
    highlight_theme: &HighlightTheme,
) -> ParsedDiff {
    let mut diff = parse_unified_diff(patch);
    apply_syntax_highlighting(file, patch, &mut diff, highlight_theme);
    diff
}

fn apply_syntax_highlighting(
    file: &DiffFile,
    patch: &str,
    diff: &mut ParsedDiff,
    highlight_theme: &HighlightTheme,
) {
    for line in diff.hunks.iter_mut().flat_map(|hunk| hunk.lines.iter_mut()) {
        line.syntax_highlights.clear();
    }

    let Some(language) = language_for_file(file) else {
        return;
    };
    if LanguageRegistry::singleton().language(language).is_none() {
        return;
    }
    if patch.len() > MAX_SYNTAX_PATCH_BYTES || has_oversized_line(diff) {
        return;
    }

    apply_syntax_highlighting_for_side(diff, language, SyntaxSide::Old, highlight_theme);
    apply_syntax_highlighting_for_side(diff, language, SyntaxSide::New, highlight_theme);
}

fn apply_syntax_highlighting_for_side(
    diff: &mut ParsedDiff,
    language: &str,
    side: SyntaxSide,
    highlight_theme: &HighlightTheme,
) {
    let (document, line_ranges) = syntax_document_for_side(diff, side);
    if document.is_empty() {
        return;
    }

    let mut highlighter = SyntaxHighlighter::new(language);
    let rope = Rope::from_str(&document);
    if !highlighter.update(None, &rope, Some(SYNTAX_PARSE_TIMEOUT)) {
        return;
    }

    let styles = highlighter.styles(&(0..document.len()), highlight_theme);
    apply_document_styles(diff, &line_ranges, styles);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SyntaxSide {
    Old,
    New,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SyntaxLineRange {
    hunk_index: usize,
    line_index: usize,
    start: usize,
    end: usize,
    attach: bool,
}

fn syntax_document_for_side(diff: &ParsedDiff, side: SyntaxSide) -> (String, Vec<SyntaxLineRange>) {
    let mut document = String::new();
    let mut line_ranges = Vec::new();

    for (hunk_index, hunk) in diff.hunks.iter().enumerate() {
        for (line_index, line) in hunk.lines.iter().enumerate() {
            let (include, attach) = match (side, line.kind) {
                (SyntaxSide::Old, DiffLineKind::Removed) => (true, true),
                (SyntaxSide::Old, DiffLineKind::Context) => (true, false),
                (SyntaxSide::New, DiffLineKind::Added | DiffLineKind::Context) => (true, true),
                _ => (false, false),
            };

            if !include {
                continue;
            }

            let start = document.len();
            document.push_str(&line.text);
            let end = document.len();
            document.push('\n');
            line_ranges.push(SyntaxLineRange {
                hunk_index,
                line_index,
                start,
                end,
                attach,
            });
        }
    }

    (document, line_ranges)
}

fn apply_document_styles(
    diff: &mut ParsedDiff,
    line_ranges: &[SyntaxLineRange],
    styles: Vec<(Range<usize>, HighlightStyle)>,
) {
    let mut first_candidate_line = 0;

    for (range, style) in styles {
        if style == HighlightStyle::default() {
            continue;
        }

        while first_candidate_line < line_ranges.len()
            && line_ranges[first_candidate_line].end <= range.start
        {
            first_candidate_line += 1;
        }

        let mut line_index = first_candidate_line;
        while let Some(line_range) = line_ranges.get(line_index) {
            if line_range.start >= range.end {
                break;
            }

            if line_range.attach {
                let start = range.start.max(line_range.start);
                let end = range.end.min(line_range.end);
                if start < end {
                    diff.hunks[line_range.hunk_index].lines[line_range.line_index]
                        .syntax_highlights
                        .push((start - line_range.start..end - line_range.start, style));
                }
            }

            line_index += 1;
        }
    }
}

fn has_oversized_line(diff: &ParsedDiff) -> bool {
    diff.hunks
        .iter()
        .flat_map(|hunk| hunk.lines.iter())
        .any(|line| line.text.len() > MAX_SYNTAX_LINE_BYTES)
}

fn language_for_file(file: &DiffFile) -> Option<&'static str> {
    language_for_path(&file.path)
        .or_else(|| file.previous_path.as_deref().and_then(language_for_path))
}

fn language_for_path(path: &str) -> Option<&'static str> {
    let path = Path::new(path);
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    match file_name.as_str() {
        "cargo.lock" => return Some("toml"),
        "makefile" | "gnumakefile" => return Some("make"),
        _ => {}
    }

    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())?
        .to_ascii_lowercase();

    match extension.as_str() {
        "rs" => Some("rust"),
        "ts" | "mts" | "cts" => Some("typescript"),
        "tsx" => Some("tsx"),
        "js" | "mjs" | "cjs" | "jsx" => Some("javascript"),
        "json" | "jsonc" => Some("json"),
        "md" | "markdown" | "mdx" => Some("markdown"),
        "toml" => Some("toml"),
        "yaml" | "yml" => Some("yaml"),
        "html" | "htm" => Some("html"),
        "css" | "scss" => Some("css"),
        "go" => Some("go"),
        "py" => Some("python"),
        "rb" => Some("ruby"),
        "java" => Some("java"),
        "kt" | "kts" | "ktm" => Some("kotlin"),
        "swift" => Some("swift"),
        "c" => Some("c"),
        "cc" | "cpp" | "cxx" | "hh" | "hpp" | "hxx" => Some("cpp"),
        "sh" | "bash" | "zsh" => Some("bash"),
        "lua" => Some("lua"),
        "php" | "php3" | "php4" | "php5" | "phtml" => Some("php"),
        "sql" => Some("sql"),
        "proto" => Some("proto"),
        "zig" => Some("zig"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use harbor_domain::{DiffFile, FileStatus};

    use super::*;

    #[test]
    fn parses_only_files_with_patches() {
        let files = vec![
            DiffFile {
                path: "src/lib.rs".to_string(),
                previous_path: None,
                status: FileStatus::Modified,
                additions: 1,
                deletions: 1,
                changes: 2,
                patch: Some("@@ -1 +1 @@\n-old\n+new\n".to_string()),
            },
            DiffFile {
                path: "assets/logo.png".to_string(),
                previous_path: None,
                status: FileStatus::Modified,
                additions: 0,
                deletions: 0,
                changes: 0,
                patch: None,
            },
        ];

        let parsed = parse_files(&files);

        assert_eq!(parsed.len(), 2);
        assert!(parsed[0].is_some());
        assert!(parsed[1].is_none());
    }

    #[test]
    fn detects_common_languages_from_paths() {
        assert_eq!(language_for_path("src/lib.rs"), Some("rust"));
        assert_eq!(language_for_path("web/app.ts"), Some("typescript"));
        assert_eq!(language_for_path("web/app.tsx"), Some("tsx"));
        assert_eq!(language_for_path("data/config.json"), Some("json"));
        assert_eq!(language_for_path("README.md"), Some("markdown"));
        assert_eq!(language_for_path("Cargo.toml"), Some("toml"));
        assert_eq!(language_for_path("settings.yaml"), Some("yaml"));
        assert_eq!(language_for_path("asset.bin"), None);
    }

    #[test]
    fn attaches_syntax_highlights_to_diff_code_lines() {
        let file = DiffFile {
            path: "src/lib.rs".to_string(),
            previous_path: None,
            status: FileStatus::Modified,
            additions: 1,
            deletions: 1,
            changes: 2,
            patch: None,
        };
        let parsed = parse_unified_diff_with_syntax(
            &file,
            "@@ -1,3 +1,3 @@\n fn main() {\n-let old_value = true;\n+let new_value = false;\n }\n",
            &HighlightTheme::default_dark(),
        );
        let lines = &parsed.hunks[0].lines;

        assert!(!lines[0].syntax_highlights.is_empty());
        assert!(!lines[1].syntax_highlights.is_empty());
        assert!(!lines[2].syntax_highlights.is_empty());
    }

    #[test]
    fn attaches_syntax_highlights_to_typescript_added_files() {
        let file = DiffFile {
            path: "src/hooks/useKeyboardInset.ts".to_string(),
            previous_path: None,
            status: FileStatus::Added,
            additions: 3,
            deletions: 0,
            changes: 3,
            patch: None,
        };
        let parsed = parse_unified_diff_with_syntax(
            &file,
            "@@ -0,0 +1,3 @@\n+import { useEffect } from 'react';\n+\n+const KEYBOARD_OVERLAY_THRESHOLD = 24;\n",
            &HighlightTheme::default_dark(),
        );
        let lines = &parsed.hunks[0].lines;

        assert!(!lines[0].syntax_highlights.is_empty());
        assert!(lines[1].syntax_highlights.is_empty());
        assert!(!lines[2].syntax_highlights.is_empty());
    }

    #[test]
    fn leaves_metadata_and_unknown_languages_plain() {
        let file = DiffFile {
            path: "notes.unknown".to_string(),
            previous_path: None,
            status: FileStatus::Modified,
            additions: 1,
            deletions: 1,
            changes: 2,
            patch: None,
        };
        let parsed = parse_unified_diff_with_syntax(
            &file,
            "@@ -1 +1 @@\n-old\n+new\n\\ No newline at end of file\n",
            &HighlightTheme::default_dark(),
        );

        assert!(
            parsed
                .hunks
                .iter()
                .flat_map(|hunk| hunk.lines.iter())
                .all(|line| line.syntax_highlights.is_empty())
        );
    }

    #[test]
    fn skips_syntax_for_missing_patches_and_oversized_lines() {
        let missing_patch = DiffFile {
            path: "src/lib.rs".to_string(),
            previous_path: None,
            status: FileStatus::Modified,
            additions: 0,
            deletions: 0,
            changes: 0,
            patch: None,
        };
        assert!(
            parse_files_with_syntax(&[missing_patch], &HighlightTheme::default_dark())[0].is_none()
        );

        let long_line = "a".repeat(MAX_SYNTAX_LINE_BYTES + 1);
        let file = DiffFile {
            path: "src/lib.rs".to_string(),
            previous_path: None,
            status: FileStatus::Modified,
            additions: 1,
            deletions: 0,
            changes: 1,
            patch: None,
        };
        let parsed = parse_unified_diff_with_syntax(
            &file,
            &format!("@@ -1 +1 @@\n+{long_line}\n"),
            &HighlightTheme::default_dark(),
        );

        assert!(parsed.hunks[0].lines[0].syntax_highlights.is_empty());
    }

    #[test]
    fn skips_syntax_for_oversized_patches() {
        let file = DiffFile {
            path: "src/lib.rs".to_string(),
            previous_path: None,
            status: FileStatus::Modified,
            additions: 1,
            deletions: 0,
            changes: 1,
            patch: None,
        };
        let filler = " context\n".repeat(MAX_SYNTAX_PATCH_BYTES / " context\n".len() + 1);
        let parsed = parse_unified_diff_with_syntax(
            &file,
            &format!("@@ -1 +1 @@\n+let value = true;\n{filler}"),
            &HighlightTheme::default_dark(),
        );

        assert!(
            parsed
                .hunks
                .iter()
                .flat_map(|hunk| hunk.lines.iter())
                .all(|line| line.syntax_highlights.is_empty())
        );
    }
}
