use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffResult {
    pub base_branch: String,
    pub compare_branch: String,
    pub files: Vec<FileDiff>,
    pub parse_errors: Vec<ParseError>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiff {
    pub path: PathBuf,
    pub status: FileStatus,
    pub language: Option<Language>,
    pub chunks: Vec<DiffChunk>,
    pub old_content: Option<String>,
    pub new_content: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    Added,
    Deleted,
    Modified,
    Renamed { similarity_percent: u8 },
    Copied,
}

impl std::fmt::Display for FileStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Added => write!(f, "A"),
            Self::Deleted => write!(f, "D"),
            Self::Modified => write!(f, "M"),
            Self::Renamed { similarity_percent } => write!(f, "R{}", similarity_percent),
            Self::Copied => write!(f, "C"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Go,
    Java,
    CSharp,
    Cpp,
    C,
    Ruby,
    Kotlin,
    Swift,
    Yaml,
    Json,
    Toml,
    Markdown,
    Shell,
    Unknown,
}

impl Language {
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "rs" => Self::Rust,
            "py" => Self::Python,
            "js" | "mjs" | "cjs" => Self::JavaScript,
            "ts" | "tsx" => Self::TypeScript,
            "go" => Self::Go,
            "java" => Self::Java,
            "cs" => Self::CSharp,
            "cpp" | "cc" | "cxx" | "hpp" => Self::Cpp,
            "c" | "h" => Self::C,
            "rb" => Self::Ruby,
            "kt" | "kts" => Self::Kotlin,
            "swift" => Self::Swift,
            "yaml" | "yml" => Self::Yaml,
            "json" => Self::Json,
            "toml" => Self::Toml,
            "md" | "markdown" => Self::Markdown,
            "sh" | "bash" | "zsh" => Self::Shell,
            _ => Self::Unknown,
        }
    }

    pub fn from_path(path: &PathBuf) -> Self {
        path.extension()
            .and_then(|e| e.to_str())
            .map(Self::from_extension)
            .unwrap_or(Self::Unknown)
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::Python => "python",
            Self::JavaScript => "javascript",
            Self::TypeScript => "typescript",
            Self::Go => "go",
            Self::Java => "java",
            Self::CSharp => "csharp",
            Self::Cpp => "cpp",
            Self::C => "c",
            Self::Ruby => "ruby",
            Self::Kotlin => "kotlin",
            Self::Swift => "swift",
            Self::Yaml => "yaml",
            Self::Json => "json",
            Self::Toml => "toml",
            Self::Markdown => "markdown",
            Self::Shell => "shell",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffChunk {
    pub id: ChunkId,
    pub old_range: LineRange,
    pub new_range: LineRange,
    pub header: String,
    pub lines: Vec<DiffLine>,
}

impl DiffChunk {
    pub fn additions(&self) -> usize {
        self.lines.iter().filter(|l| l.kind == LineKind::Add).count()
    }

    pub fn deletions(&self) -> usize {
        self.lines.iter().filter(|l| l.kind == LineKind::Remove).count()
    }

    pub fn changes(&self) -> usize {
        self.additions() + self.deletions()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkId(pub u64);

impl std::fmt::Display for ChunkId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineRange {
    pub start: u32,
    pub count: u32,
}

impl LineRange {
    pub fn end(&self) -> u32 {
        self.start + self.count.saturating_sub(1)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    pub kind: LineKind,
    pub old_line_num: Option<u32>,
    pub new_line_num: Option<u32>,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    Context,
    Add,
    Remove,
}

impl LineKind {
    pub fn prefix(&self) -> char {
        match self {
            Self::Context => ' ',
            Self::Add => '+',
            Self::Remove => '-',
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub file_path: PathBuf,
    pub message: String,
    pub line: Option<u32>,
}
