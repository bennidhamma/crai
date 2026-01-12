# crai

<img width="1903" height="1042" alt="image" src="https://github.com/user-attachments/assets/51f31136-998c-45cc-9861-5d196cc188e2" />
<img width="1913" height="1046" alt="image" src="https://github.com/user-attachments/assets/8b17ef4e-c516-4dbf-b7de-2daa64771c5a" />


Code review is crai-zee hard. Most diffs are full of noise—imports, formatting, lock files, trivial refactors. The interesting parts (logic changes, security implications, subtle bugs) get buried.

crai uses AI to score each chunk of a diff by how "controversial" or review-worthy it is, then filters out the noise so you can focus on what actually matters. It's a TUI that shows you diffs alongside AI analysis, helping you review code faster and catch things you might miss.

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
