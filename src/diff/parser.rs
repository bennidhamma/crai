use crate::diff::chunk::{
    ChunkId, DiffChunk, DiffLine, DiffResult, FileDiff, FileStatus, Language, LineKind, LineRange,
    ParseError,
};
use crate::diff::git::GitOperations;
use crate::error::CraiResult;
use std::path::PathBuf;

pub struct DiffParser {
    git: GitOperations,
    context_lines: u32,
}

impl DiffParser {
    pub fn new(repo_path: PathBuf, context_lines: u32) -> Self {
        Self {
            git: GitOperations::new(repo_path),
            context_lines,
        }
    }

    pub async fn parse_branches(
        &self,
        base_branch: &str,
        compare_branch: &str,
    ) -> CraiResult<DiffResult> {
        // Get unified diff for all files
        let unified_diff = self
            .git
            .get_unified_diff(base_branch, compare_branch, self.context_lines)
            .await?;

        let (files, parse_errors) = self.parse_unified_diff(&unified_diff)?;

        Ok(DiffResult {
            base_branch: base_branch.to_string(),
            compare_branch: compare_branch.to_string(),
            files,
            parse_errors,
        })
    }

    /// Parse unstaged changes (working directory vs HEAD)
    pub async fn parse_unstaged(&self) -> CraiResult<DiffResult> {
        let unified_diff = self
            .git
            .get_unstaged_unified_diff(self.context_lines)
            .await?;

        let (files, parse_errors) = self.parse_unified_diff(&unified_diff)?;

        Ok(DiffResult {
            base_branch: "HEAD".to_string(),
            compare_branch: "(working directory)".to_string(),
            files,
            parse_errors,
        })
    }

    /// Parse staged changes (index vs HEAD)
    pub async fn parse_staged(&self) -> CraiResult<DiffResult> {
        let unified_diff = self
            .git
            .get_staged_unified_diff(self.context_lines)
            .await?;

        let (files, parse_errors) = self.parse_unified_diff(&unified_diff)?;

        Ok(DiffResult {
            base_branch: "HEAD".to_string(),
            compare_branch: "(staged)".to_string(),
            files,
            parse_errors,
        })
    }

    fn parse_unified_diff(&self, diff_text: &str) -> CraiResult<(Vec<FileDiff>, Vec<ParseError>)> {
        let mut files = Vec::new();
        let mut errors = Vec::new();
        let mut chunk_id_counter = 0u64;

        let mut current_file: Option<FileDiffBuilder> = None;

        for line in diff_text.lines() {
            if line.starts_with("diff --git") {
                // Finish previous file if any
                if let Some(builder) = current_file.take() {
                    files.push(builder.build());
                }

                // Start new file
                let path = parse_diff_header(line);
                current_file = Some(FileDiffBuilder::new(path));
            } else if let Some(ref mut builder) = current_file {
                if line.starts_with("---") {
                    // Old file path (we already have it from diff --git)
                    if line == "--- /dev/null" {
                        builder.status = FileStatus::Added;
                    }
                } else if line.starts_with("+++") {
                    // New file path
                    if line == "+++ /dev/null" {
                        builder.status = FileStatus::Deleted;
                    }
                } else if line.starts_with("@@") {
                    // Chunk header
                    if let Some((old_range, new_range, header)) = parse_chunk_header(line) {
                        builder.start_chunk(ChunkId(chunk_id_counter), old_range, new_range, header);
                        chunk_id_counter += 1;
                    } else {
                        errors.push(ParseError {
                            file_path: builder.path.clone(),
                            message: format!("Failed to parse chunk header: {}", line),
                            line: None,
                        });
                    }
                } else if line.starts_with('+') && !line.starts_with("+++") {
                    builder.add_line(LineKind::Add, &line[1..]);
                } else if line.starts_with('-') && !line.starts_with("---") {
                    builder.add_line(LineKind::Remove, &line[1..]);
                } else if line.starts_with(' ') || line.is_empty() {
                    let content = if line.is_empty() { "" } else { &line[1..] };
                    builder.add_line(LineKind::Context, content);
                } else if line.starts_with('\\') {
                    // "\ No newline at end of file" - skip
                }
            }
        }

        // Don't forget the last file
        if let Some(builder) = current_file {
            files.push(builder.build());
        }

        Ok((files, errors))
    }
}

struct FileDiffBuilder {
    path: PathBuf,
    status: FileStatus,
    language: Language,
    chunks: Vec<DiffChunk>,
    current_chunk: Option<ChunkBuilder>,
}

impl FileDiffBuilder {
    fn new(path: PathBuf) -> Self {
        let language = Language::from_path(&path);
        Self {
            path,
            status: FileStatus::Modified,
            language,
            chunks: Vec::new(),
            current_chunk: None,
        }
    }

    fn start_chunk(&mut self, id: ChunkId, old_range: LineRange, new_range: LineRange, header: String) {
        // Finish previous chunk if any
        if let Some(chunk) = self.current_chunk.take() {
            self.chunks.push(chunk.build());
        }

        self.current_chunk = Some(ChunkBuilder {
            id,
            old_range,
            new_range,
            header,
            lines: Vec::new(),
            current_old_line: old_range.start,
            current_new_line: new_range.start,
        });
    }

    fn add_line(&mut self, kind: LineKind, content: &str) {
        if let Some(ref mut chunk) = self.current_chunk {
            chunk.add_line(kind, content);
        }
    }

    fn build(mut self) -> FileDiff {
        // Finish current chunk if any
        if let Some(chunk) = self.current_chunk.take() {
            self.chunks.push(chunk.build());
        }

        FileDiff {
            path: self.path,
            status: self.status,
            language: Some(self.language),
            chunks: self.chunks,
            old_content: None,
            new_content: None,
        }
    }
}

struct ChunkBuilder {
    id: ChunkId,
    old_range: LineRange,
    new_range: LineRange,
    header: String,
    lines: Vec<DiffLine>,
    current_old_line: u32,
    current_new_line: u32,
}

impl ChunkBuilder {
    fn add_line(&mut self, kind: LineKind, content: &str) {
        let (old_num, new_num) = match kind {
            LineKind::Context => {
                let old = self.current_old_line;
                let new = self.current_new_line;
                self.current_old_line += 1;
                self.current_new_line += 1;
                (Some(old), Some(new))
            }
            LineKind::Add => {
                let new = self.current_new_line;
                self.current_new_line += 1;
                (None, Some(new))
            }
            LineKind::Remove => {
                let old = self.current_old_line;
                self.current_old_line += 1;
                (Some(old), None)
            }
        };

        self.lines.push(DiffLine {
            kind,
            old_line_num: old_num,
            new_line_num: new_num,
            content: content.to_string(),
        });
    }

    fn build(self) -> DiffChunk {
        DiffChunk {
            id: self.id,
            old_range: self.old_range,
            new_range: self.new_range,
            header: self.header,
            lines: self.lines,
        }
    }
}

fn parse_diff_header(line: &str) -> PathBuf {
    // Format: "diff --git a/path/to/file b/path/to/file"
    if let Some(rest) = line.strip_prefix("diff --git ") {
        // Try to find "b/" which marks the new path
        if let Some(b_idx) = rest.find(" b/") {
            let new_path = &rest[b_idx + 3..];
            return PathBuf::from(new_path);
        }
    }
    // Fallback: just use what we can find
    PathBuf::from("unknown")
}

fn parse_chunk_header(line: &str) -> Option<(LineRange, LineRange, String)> {
    // Format: "@@ -old_start,old_count +new_start,new_count @@ optional function context"
    let line = line.strip_prefix("@@ ")?;
    let end_idx = line.find(" @@")?;
    let ranges = &line[..end_idx];
    let header = line[end_idx + 3..].trim().to_string();

    let mut parts = ranges.split(' ');
    let old_range = parse_range(parts.next()?.strip_prefix('-')?)?;
    let new_range = parse_range(parts.next()?.strip_prefix('+')?)?;

    Some((old_range, new_range, header))
}

fn parse_range(s: &str) -> Option<LineRange> {
    if let Some((start, count)) = s.split_once(',') {
        Some(LineRange {
            start: start.parse().ok()?,
            count: count.parse().ok()?,
        })
    } else {
        // Single line change
        Some(LineRange {
            start: s.parse().ok()?,
            count: 1,
        })
    }
}
