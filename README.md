# crai

AI-powered code review tool with a terminal UI.

## Features

- **AI-assisted review**: Uses Claude, OpenAI, or custom AI providers to score and analyze code changes
- **Smart filtering**: Automatically filters out noise (whitespace, imports, generated files, lock files)
- **Terminal UI**: Browse diffs with AI analysis in a ratatui-based interface
- **Flexible diff modes**: Compare branches, staged changes, or working directory changes
- **Subagents**: Specialized reviewers for security, performance, and usability concerns

## Installation

```bash
cargo install --path .
```

## Usage

```bash
# Review unstaged changes (default)
crai

# Review staged changes
crai --staged

# Compare branches
crai --base main --compare feature-branch

# Non-interactive summary
crai summary

# Check dependencies
crai doctor

# Generate config file
crai init
```

## Configuration

Copy `crai.toml.example` to `crai.toml` and customize:

```toml
[ai]
provider = "claude"  # claude, openai, or custom
concurrent_requests = 4

[filters]
controversiality_threshold = 0.3  # Filter out low-concern chunks
auto_filter_imports = true
auto_filter_whitespace = true

[subagents.security]
enabled = true
priority_threshold = 0.5
```

## Requirements

- Git
- An AI provider CLI (e.g., `claude` CLI for Claude)
- Optional: [difftastic](https://difftastic.wilfred.me.uk/) for semantic diffs

## License

MIT
