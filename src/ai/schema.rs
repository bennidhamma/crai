use serde::{Deserialize, Serialize};

/// Structured response from AI for controversiality scoring
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ControversialityResponse {
    pub score: f64,
    pub classification: ChangeClassification,
    pub reasoning: String,
    pub concerns: Vec<Concern>,
    pub review_depth: ReviewDepth,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChangeClassification {
    Trivial,
    Routine,
    Notable,
    Significant,
    Critical,
}

impl ChangeClassification {
    pub fn color(&self) -> &'static str {
        match self {
            Self::Trivial => "gray",
            Self::Routine => "green",
            Self::Notable => "yellow",
            Self::Significant => "orange",
            Self::Critical => "red",
        }
    }
}

impl std::fmt::Display for ChangeClassification {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Trivial => write!(f, "TRIVIAL"),
            Self::Routine => write!(f, "ROUTINE"),
            Self::Notable => write!(f, "NOTABLE"),
            Self::Significant => write!(f, "SIGNIFICANT"),
            Self::Critical => write!(f, "CRITICAL"),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Concern {
    pub category: ConcernCategory,
    pub description: String,
    pub severity: Severity,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConcernCategory {
    Security,
    Performance,
    Correctness,
    Maintainability,
    Readability,
    Testing,
    Documentation,
    Architecture,
}

impl std::fmt::Display for ConcernCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Security => write!(f, "Security"),
            Self::Performance => write!(f, "Performance"),
            Self::Correctness => write!(f, "Correctness"),
            Self::Maintainability => write!(f, "Maintainability"),
            Self::Readability => write!(f, "Readability"),
            Self::Testing => write!(f, "Testing"),
            Self::Documentation => write!(f, "Documentation"),
            Self::Architecture => write!(f, "Architecture"),
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Low => write!(f, "LOW"),
            Self::Medium => write!(f, "MED"),
            Self::High => write!(f, "HIGH"),
            Self::Critical => write!(f, "CRIT"),
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReviewDepth {
    Skip,
    Glance,
    Review,
    DeepDive,
}

impl std::fmt::Display for ReviewDepth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Skip => write!(f, "Skip"),
            Self::Glance => write!(f, "Glance"),
            Self::Review => write!(f, "Review"),
            Self::DeepDive => write!(f, "Deep Dive"),
        }
    }
}

/// JSON Schema for subagent review response
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SubagentReviewResponse {
    pub findings: Vec<Finding>,
    pub overall_assessment: OverallAssessment,
    pub recommendations: Vec<Recommendation>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Finding {
    pub id: String,
    pub title: String,
    pub description: String,
    pub location: FindingLocation,
    pub severity: Severity,
    pub category: ConcernCategory,
    pub code_snippet: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FindingLocation {
    pub file_path: String,
    pub line_start: u32,
    pub line_end: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OverallAssessment {
    pub risk_level: RiskLevel,
    pub summary: String,
    pub areas_of_concern: Vec<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

impl std::fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Low => write!(f, "Low"),
            Self::Medium => write!(f, "Medium"),
            Self::High => write!(f, "High"),
            Self::Critical => write!(f, "Critical"),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Recommendation {
    pub priority: Priority,
    pub action: String,
    pub rationale: String,
    pub affected_files: Vec<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    Optional,
    Suggested,
    Recommended,
    Required,
}

impl std::fmt::Display for Priority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Optional => write!(f, "Optional"),
            Self::Suggested => write!(f, "Suggested"),
            Self::Recommended => write!(f, "Recommended"),
            Self::Required => write!(f, "Required"),
        }
    }
}

/// Summary response from AI
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SummaryResponse {
    pub overview: String,
    pub key_changes: Vec<KeyChange>,
    pub risk_assessment: RiskAssessment,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KeyChange {
    pub description: String,
    pub affected_files: Vec<String>,
    pub impact_level: ImpactLevel,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ImpactLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RiskAssessment {
    pub overall_risk: RiskLevel,
    pub factors: Vec<RiskFactor>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RiskFactor {
    pub factor: String,
    pub contribution: f64,
}

// JSON Schema definitions for structured output

pub fn controversiality_json_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "score": {
                "type": "number",
                "minimum": 0.0,
                "maximum": 1.0,
                "description": "Controversiality score from 0 (trivial) to 1 (critical)"
            },
            "classification": {
                "type": "string",
                "enum": ["trivial", "routine", "notable", "significant", "critical"]
            },
            "reasoning": {
                "type": "string",
                "description": "Brief explanation of the score"
            },
            "concerns": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "category": {
                            "type": "string",
                            "enum": ["security", "performance", "correctness", "maintainability",
                                     "readability", "testing", "documentation", "architecture"]
                        },
                        "description": { "type": "string" },
                        "severity": {
                            "type": "string",
                            "enum": ["low", "medium", "high", "critical"]
                        }
                    },
                    "required": ["category", "description", "severity"]
                }
            },
            "review_depth": {
                "type": "string",
                "enum": ["skip", "glance", "review", "deep_dive"]
            }
        },
        "required": ["score", "classification", "reasoning", "concerns", "review_depth"]
    })
}

pub fn subagent_review_json_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "findings": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "string" },
                        "title": { "type": "string" },
                        "description": { "type": "string" },
                        "location": {
                            "type": "object",
                            "properties": {
                                "file_path": { "type": "string" },
                                "line_start": { "type": "integer" },
                                "line_end": { "type": "integer" }
                            },
                            "required": ["file_path", "line_start"]
                        },
                        "severity": { "type": "string", "enum": ["low", "medium", "high", "critical"] },
                        "category": { "type": "string" },
                        "code_snippet": { "type": "string" }
                    },
                    "required": ["id", "title", "description", "location", "severity", "category"]
                }
            },
            "overall_assessment": {
                "type": "object",
                "properties": {
                    "risk_level": { "type": "string", "enum": ["low", "medium", "high", "critical"] },
                    "summary": { "type": "string" },
                    "areas_of_concern": { "type": "array", "items": { "type": "string" } }
                },
                "required": ["risk_level", "summary", "areas_of_concern"]
            },
            "recommendations": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "priority": { "type": "string", "enum": ["optional", "suggested", "recommended", "required"] },
                        "action": { "type": "string" },
                        "rationale": { "type": "string" },
                        "affected_files": { "type": "array", "items": { "type": "string" } }
                    },
                    "required": ["priority", "action", "rationale", "affected_files"]
                }
            }
        },
        "required": ["findings", "overall_assessment", "recommendations"]
    })
}

pub fn summary_json_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "overview": {
                "type": "string",
                "description": "High-level summary of the changes"
            },
            "key_changes": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "description": { "type": "string" },
                        "affected_files": { "type": "array", "items": { "type": "string" } },
                        "impact_level": { "type": "string", "enum": ["low", "medium", "high"] }
                    },
                    "required": ["description", "affected_files", "impact_level"]
                }
            },
            "risk_assessment": {
                "type": "object",
                "properties": {
                    "overall_risk": { "type": "string", "enum": ["low", "medium", "high", "critical"] },
                    "factors": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "factor": { "type": "string" },
                                "contribution": { "type": "number" }
                            },
                            "required": ["factor", "contribution"]
                        }
                    }
                },
                "required": ["overall_risk", "factors"]
            }
        },
        "required": ["overview", "key_changes", "risk_assessment"]
    })
}
