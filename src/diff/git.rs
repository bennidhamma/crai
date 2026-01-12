use crate::diff::chunk::{FileStatus, Language};
use crate::error::{CraiError, CraiResult};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;

pub struct GitOperations {
    repo_path: PathBuf,
}

impl GitOperations {
    pub fn new(repo_path: PathBuf) -> Self {
        Self { repo_path }
    }

    pub async fn verify_repository(&self) -> CraiResult<()> {
        let output = Command::new("git")
            .args(["-C", &self.repo_path.to_string_lossy(), "rev-parse", "--git-dir"])
            .output()
            .await?;

        if !output.status.success() {
            return Err(CraiError::NotAGitRepository(self.repo_path.clone()));
        }

        Ok(())
    }

    pub async fn verify_branch(&self, branch: &str) -> CraiResult<()> {
        let output = Command::new("git")
            .args([
                "-C",
                &self.repo_path.to_string_lossy(),
                "rev-parse",
                "--verify",
                branch,
            ])
            .output()
            .await?;

        if !output.status.success() {
            return Err(CraiError::BranchNotFound(branch.to_string()));
        }

        Ok(())
    }

    pub async fn get_current_branch(&self) -> CraiResult<String> {
        let output = Command::new("git")
            .args([
                "-C",
                &self.repo_path.to_string_lossy(),
                "rev-parse",
                "--abbrev-ref",
                "HEAD",
            ])
            .output()
            .await?;

        if !output.status.success() {
            return Err(CraiError::Git("Failed to get current branch".to_string()));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    pub async fn get_changed_files(
        &self,
        base: &str,
        compare: &str,
    ) -> CraiResult<Vec<ChangedFile>> {
        let output = Command::new("git")
            .args([
                "-C",
                &self.repo_path.to_string_lossy(),
                "diff",
                "--no-ext-diff",
                "--name-status",
                &format!("{}..{}", base, compare),
            ])
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(CraiError::Git(stderr.to_string()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut files = Vec::new();

        for line in stdout.lines() {
            if let Some(file) = parse_name_status_line(line) {
                files.push(file);
            }
        }

        Ok(files)
    }

    /// Get unstaged changes (working directory vs HEAD)
    pub async fn get_unstaged_changed_files(&self) -> CraiResult<Vec<ChangedFile>> {
        let output = Command::new("git")
            .args([
                "-C",
                &self.repo_path.to_string_lossy(),
                "diff",
                "--no-ext-diff",
                "--name-status",
                "HEAD",
            ])
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(CraiError::Git(stderr.to_string()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut files = Vec::new();

        for line in stdout.lines() {
            if let Some(file) = parse_name_status_line(line) {
                files.push(file);
            }
        }

        Ok(files)
    }

    /// Get staged changes (index vs HEAD)
    pub async fn get_staged_changed_files(&self) -> CraiResult<Vec<ChangedFile>> {
        let output = Command::new("git")
            .args([
                "-C",
                &self.repo_path.to_string_lossy(),
                "diff",
                "--no-ext-diff",
                "--name-status",
                "--cached",
                "HEAD",
            ])
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(CraiError::Git(stderr.to_string()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut files = Vec::new();

        for line in stdout.lines() {
            if let Some(file) = parse_name_status_line(line) {
                files.push(file);
            }
        }

        Ok(files)
    }

    pub async fn get_file_at_ref(&self, git_ref: &str, file_path: &Path) -> CraiResult<Option<String>> {
        let output = Command::new("git")
            .args([
                "-C",
                &self.repo_path.to_string_lossy(),
                "show",
                &format!("{}:{}", git_ref, file_path.display()),
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;

        if output.status.success() {
            Ok(Some(String::from_utf8_lossy(&output.stdout).into_owned()))
        } else {
            // File doesn't exist at this ref
            Ok(None)
        }
    }

    pub async fn get_unified_diff(
        &self,
        base: &str,
        compare: &str,
        context_lines: u32,
    ) -> CraiResult<String> {
        let output = Command::new("git")
            .args([
                "-C",
                &self.repo_path.to_string_lossy(),
                "diff",
                "--no-ext-diff",
                &format!("-U{}", context_lines),
                &format!("{}..{}", base, compare),
            ])
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(CraiError::Git(stderr.to_string()));
        }

        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    /// Get unstaged unified diff (working directory vs HEAD)
    pub async fn get_unstaged_unified_diff(&self, context_lines: u32) -> CraiResult<String> {
        let output = Command::new("git")
            .args([
                "-C",
                &self.repo_path.to_string_lossy(),
                "diff",
                "--no-ext-diff",
                &format!("-U{}", context_lines),
                "HEAD",
            ])
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(CraiError::Git(stderr.to_string()));
        }

        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    /// Get staged unified diff (index vs HEAD)
    pub async fn get_staged_unified_diff(&self, context_lines: u32) -> CraiResult<String> {
        let output = Command::new("git")
            .args([
                "-C",
                &self.repo_path.to_string_lossy(),
                "diff",
                "--no-ext-diff",
                "--cached",
                &format!("-U{}", context_lines),
                "HEAD",
            ])
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(CraiError::Git(stderr.to_string()));
        }

        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    pub async fn get_file_diff(
        &self,
        base: &str,
        compare: &str,
        file_path: &Path,
        context_lines: u32,
    ) -> CraiResult<String> {
        let output = Command::new("git")
            .args([
                "-C",
                &self.repo_path.to_string_lossy(),
                "diff",
                "--no-ext-diff",
                &format!("-U{}", context_lines),
                &format!("{}..{}", base, compare),
                "--",
                &file_path.to_string_lossy(),
            ])
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(CraiError::Git(stderr.to_string()));
        }

        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    pub async fn get_commit_messages(&self, base: &str, compare: &str) -> CraiResult<Vec<String>> {
        let output = Command::new("git")
            .args([
                "-C",
                &self.repo_path.to_string_lossy(),
                "log",
                "--format=%s",
                &format!("{}..{}", base, compare),
            ])
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(CraiError::Git(stderr.to_string()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.lines().map(|s| s.to_string()).collect())
    }
}

#[derive(Debug, Clone)]
pub struct ChangedFile {
    pub path: PathBuf,
    pub old_path: Option<PathBuf>,
    pub status: FileStatus,
    pub language: Language,
}

fn parse_name_status_line(line: &str) -> Option<ChangedFile> {
    let mut parts = line.split('\t');
    let status_str = parts.next()?.trim();
    let path_str = parts.next()?.trim();

    let (status, old_path) = if status_str.starts_with('R') {
        let similarity = status_str[1..].parse().unwrap_or(100);
        let old = PathBuf::from(path_str);
        let _new_path = parts.next()?.trim();
        (
            FileStatus::Renamed { similarity_percent: similarity },
            Some(old),
        )
    } else {
        let status = match status_str.chars().next()? {
            'A' => FileStatus::Added,
            'D' => FileStatus::Deleted,
            'M' => FileStatus::Modified,
            'C' => FileStatus::Copied,
            _ => FileStatus::Modified,
        };
        (status, None)
    };

    let final_path = if old_path.is_some() {
        // For renames, the new path is the third column
        parts.next().map(|s| PathBuf::from(s.trim())).unwrap_or_else(|| PathBuf::from(path_str))
    } else {
        PathBuf::from(path_str)
    };

    let language = Language::from_path(&final_path);

    Some(ChangedFile {
        path: final_path,
        old_path,
        status,
        language,
    })
}
