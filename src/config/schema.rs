use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    pub general: GeneralConfig,
    pub ai: AiConfig,
    pub diff: DiffConfig,
    pub filters: FilterConfig,
    pub tui: TuiConfig,
    pub subagents: SubagentConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig::default(),
            ai: AiConfig::default(),
            diff: DiffConfig::default(),
            filters: FilterConfig::default(),
            tui: TuiConfig::default(),
            subagents: SubagentConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct GeneralConfig {
    pub repository_path: PathBuf,
    pub default_base_branch: String,
    pub cache_directory: PathBuf,
    pub log_level: LogLevel,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            repository_path: PathBuf::from("."),
            default_base_branch: "main".to_string(),
            cache_directory: dirs::cache_dir()
                .unwrap_or_else(|| PathBuf::from(".cache"))
                .join("crai"),
            log_level: LogLevel::Info,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Error,
    Warn,
    #[default]
    Info,
    Debug,
    Trace,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct AiConfig {
    pub provider: AiProviderType,
    pub model: Option<String>,
    pub timeout_seconds: u64,
    pub max_retries: u32,
    pub concurrent_requests: usize,
    pub custom_cli_path: Option<PathBuf>,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            provider: AiProviderType::Claude,
            model: None,
            timeout_seconds: 60,
            max_retries: 3,
            concurrent_requests: 4,
            custom_cli_path: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum AiProviderType {
    Claude,
    #[default]
    Kiro,
    OpenAi,
    Custom,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct DiffConfig {
    pub difft_path: PathBuf,
    pub context_lines: u32,
    pub ignore_whitespace: bool,
    pub ignore_comments: bool,
    pub max_file_size_bytes: u64,
}

impl Default for DiffConfig {
    fn default() -> Self {
        Self {
            difft_path: PathBuf::from("difft"),
            context_lines: 3,
            ignore_whitespace: false,
            ignore_comments: false,
            max_file_size_bytes: 1_000_000,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct FilterConfig {
    pub auto_filter_whitespace: bool,
    pub auto_filter_imports: bool,
    pub auto_filter_renames: bool,
    pub auto_filter_generated: bool,
    pub generated_file_patterns: Vec<String>,
    pub import_patterns: HashMap<String, Vec<String>>,
    pub controversiality_threshold: f64,
}

impl Default for FilterConfig {
    fn default() -> Self {
        Self {
            auto_filter_whitespace: true,
            auto_filter_imports: true,
            auto_filter_renames: true,
            auto_filter_generated: true,
            generated_file_patterns: vec![
                r".*\.lock$".to_string(),
                r".*\.generated\..*".to_string(),
                r".*/generated/.*".to_string(),
                r".*/vendor/.*".to_string(),
                r"package-lock\.json$".to_string(),
                r"yarn\.lock$".to_string(),
                r"Cargo\.lock$".to_string(),
            ],
            import_patterns: default_import_patterns(),
            controversiality_threshold: 0.3,
        }
    }
}

fn default_import_patterns() -> HashMap<String, Vec<String>> {
    let mut patterns = HashMap::new();
    patterns.insert(
        "rust".to_string(),
        vec![r"^\s*use\s+".to_string(), r"^\s*mod\s+\w+;".to_string()],
    );
    patterns.insert(
        "python".to_string(),
        vec![
            r"^\s*import\s+".to_string(),
            r"^\s*from\s+\S+\s+import\s+".to_string(),
        ],
    );
    patterns.insert(
        "javascript".to_string(),
        vec![
            r#"^\s*import\s+.+\s+from\s+"#.to_string(),
            r"^\s*const\s+.+=\s*require\(".to_string(),
            r"^\s*export\s+\{".to_string(),
        ],
    );
    patterns.insert(
        "typescript".to_string(),
        vec![
            r#"^\s*import\s+.+\s+from\s+"#.to_string(),
            r"^\s*import\s+type\s+".to_string(),
            r"^\s*export\s+\{".to_string(),
        ],
    );
    patterns.insert(
        "go".to_string(),
        vec![r#"^\s*import\s+"#.to_string(), r"^\s*import\s+\(".to_string()],
    );
    patterns.insert(
        "java".to_string(),
        vec![r"^\s*import\s+".to_string(), r"^\s*package\s+".to_string()],
    );
    patterns
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct TuiConfig {
    pub color_scheme: ColorScheme,
    pub show_line_numbers: bool,
    pub diff_tab_width: u8,
    pub analysis_pane_width_percent: u8,
}

impl Default for TuiConfig {
    fn default() -> Self {
        Self {
            color_scheme: ColorScheme::Dark,
            show_line_numbers: true,
            diff_tab_width: 4,
            analysis_pane_width_percent: 35,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ColorScheme {
    #[default]
    Dark,
    Light,
    HighContrast,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct SubagentConfig {
    pub security: SubagentSettings,
    pub performance: SubagentSettings,
    pub usability: SubagentSettings,
}

impl Default for SubagentConfig {
    fn default() -> Self {
        Self {
            security: SubagentSettings {
                enabled: true,
                model: None,
                custom_prompt: None,
                priority_threshold: 0.5,
            },
            performance: SubagentSettings {
                enabled: true,
                model: None,
                custom_prompt: None,
                priority_threshold: 0.6,
            },
            usability: SubagentSettings {
                enabled: false,
                model: None,
                custom_prompt: None,
                priority_threshold: 0.7,
            },
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SubagentSettings {
    pub enabled: bool,
    pub model: Option<String>,
    pub custom_prompt: Option<String>,
    pub priority_threshold: f64,
}
