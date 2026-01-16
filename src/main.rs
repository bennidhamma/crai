use clap::{Parser, Subcommand};
use crai::ai::provider::{AiProviderFactory, ScoringContext, SummaryContext};
use crai::ai::scoring::{ScoringOrchestrator, ScoringUpdate};
use crai::config::{self, AiProviderType, Config};
use crai::diff::filter::ChunkFilter;
use crai::diff::git::GitOperations;
use crai::diff::parser::DiffParser;
use crai::error::CraiResult;
use crai::tui::event::{Action, Event, EventHandler};
use crai::tui::layout::LayoutManager;
use crai::tui::{self, App};
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Parser)]
#[command(name = "crai")]
#[command(about = "AI-powered code review tool")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Path to config file (defaults to ~/.config/crai/crai.toml)
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Base branch to compare against
    #[arg(short, long)]
    base: Option<String>,

    /// Compare branch (defaults to HEAD)
    #[arg(short = 'C', long)]
    compare: Option<String>,

    /// Compare staged changes against HEAD
    #[arg(long, conflicts_with_all = ["base", "compare", "unstaged"])]
    staged: bool,

    /// Compare unstaged changes against HEAD (default if no args)
    #[arg(long, conflicts_with_all = ["base", "compare", "staged"])]
    unstaged: bool,

    /// Repository path
    #[arg(short, long, default_value = ".")]
    repo: PathBuf,

    /// Skip AI analysis (use heuristics only)
    #[arg(long)]
    no_ai: bool,

    /// Output format for non-interactive mode (text, json)
    #[arg(long, default_value = "text")]
    format: String,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new config file
    Init {
        /// Output path for config (defaults to ~/.config/crai/crai.toml)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Check configuration and dependencies
    Doctor,

    /// Show summary only (non-interactive)
    Summary,
}

#[tokio::main]
async fn main() -> CraiResult<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::WARN.into()),
        )
        .init();

    let cli = Cli::parse();

    // Handle Init command before loading config
    if let Some(Commands::Init { output }) = &cli.command {
        let output_path = output
            .clone()
            .or_else(config::default_config_path)
            .ok_or_else(|| crai::error::CraiError::Config(
                "Could not determine config directory".to_string()
            ))?;

        // Run interactive setup
        let provider = prompt_for_provider()?;
        config::create_config_with_provider(&output_path, provider)?;
        println!("Created config at {}", output_path.display());
        return Ok(());
    }

    // Determine config path: CLI arg > user config dir > default
    let config_path = cli.config.clone()
        .or_else(config::default_config_path);

    // Load config or run first-time setup
    let config = match &config_path {
        Some(path) if path.exists() => config::load_config(path)?,
        Some(path) => {
            // Config path specified but doesn't exist - run setup
            println!("Welcome to CRAI - AI-powered Code Review\n");
            println!("No configuration found. Let's set up your config.\n");

            let provider = prompt_for_provider()?;
            config::create_config_with_provider(path, provider)?;
            println!("\nConfig created at {}\n", path.display());

            config::load_config(path)?
        }
        None => {
            // No config path available - use defaults
            eprintln!("Warning: Could not determine config directory, using defaults");
            Config::default()
        }
    };

    match cli.command {
        Some(Commands::Init { .. }) => unreachable!(), // Already handled above
        Some(Commands::Doctor) => run_doctor(&config).await,
        Some(Commands::Summary) => run_summary(&cli, &config).await,
        None => run_interactive(&cli, &config).await,
    }
}

/// Prompt user to select an AI provider
fn prompt_for_provider() -> CraiResult<AiProviderType> {
    println!("Which AI CLI would you like to use?\n");
    println!("  1. kiro-cli  - Kiro CLI (default)");
    println!("  2. claude    - Anthropic's Claude CLI\n");

    print!("Enter choice [1]: ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim();

    let provider = match input {
        "" | "1" | "kiro" | "kiro-cli" => {
            println!("\nSelected: Kiro CLI");
            println!("Make sure 'kiro-cli' is installed and in your PATH.");
            AiProviderType::Kiro
        }
        "2" | "claude" => {
            println!("\nSelected: Claude CLI");
            println!("Make sure 'claude' is installed and in your PATH.");
            AiProviderType::Claude
        }
        _ => {
            println!("\nUnknown option '{}', defaulting to Kiro CLI", input);
            AiProviderType::Kiro
        }
    };

    Ok(provider)
}

async fn run_doctor(config: &Config) -> CraiResult<()> {
    println!("CRAI - Code Review AI - Dependency Check\n");

    // Check git
    print!("git: ");
    match tokio::process::Command::new("git")
        .args(["--version"])
        .output()
        .await
    {
        Ok(output) if output.status.success() => {
            println!("OK ({})", String::from_utf8_lossy(&output.stdout).trim());
        }
        _ => println!("NOT FOUND"),
    }

    // Check difft
    print!("difftastic: ");
    match tokio::process::Command::new(&config.diff.difft_path)
        .args(["--version"])
        .output()
        .await
    {
        Ok(output) if output.status.success() => {
            println!("OK ({})", String::from_utf8_lossy(&output.stdout).trim());
        }
        _ => println!("NOT FOUND (optional, for semantic diffs)"),
    }

    // Check AI provider
    print!("AI provider ({:?}): ", config.ai.provider);
    match AiProviderFactory::create(&config.ai) {
        Ok(provider) => match provider.health_check().await {
            Ok(health) if health.is_available => {
                println!(
                    "OK ({})",
                    health.cli_version.unwrap_or_else(|| "version unknown".to_string())
                );
            }
            Ok(_) => println!("NOT AVAILABLE"),
            Err(e) => println!("ERROR: {}", e),
        },
        Err(e) => println!("ERROR: {}", e),
    }

    println!("\nConfiguration:");
    let config_path = config::default_config_path();
    let config_status = match &config_path {
        Some(p) if p.exists() => format!("Found at {}", p.display()),
        Some(p) => format!("Not found (expected at {})", p.display()),
        None => "Could not determine config directory".to_string(),
    };
    println!("  Config file: {}", config_status);
    println!("  AI provider: {:?}", config.ai.provider);
    println!("  Controversiality threshold: {}", config.filters.controversiality_threshold);
    println!("  Concurrent AI requests: {}", config.ai.concurrent_requests);

    Ok(())
}

async fn run_summary(cli: &Cli, config: &Config) -> CraiResult<()> {
    let git = GitOperations::new(cli.repo.clone());
    git.verify_repository().await?;

    let parser = DiffParser::new(cli.repo.clone(), config.diff.context_lines);

    let diff_result = if cli.staged {
        let result = parser.parse_staged().await?;
        println!("CRAI Summary: HEAD -> (staged)\n");
        result
    } else if cli.unstaged || (cli.base.is_none() && cli.compare.is_none()) {
        // Default to unstaged if no flags or branches specified
        let result = parser.parse_unstaged().await?;
        println!("CRAI Summary: HEAD -> (working directory)\n");
        result
    } else {
        let base = cli
            .base
            .as_deref()
            .unwrap_or(&config.general.default_base_branch);
        let compare = cli.compare.as_deref().unwrap_or("HEAD");

        git.verify_branch(base).await?;
        git.verify_branch(compare).await?;

        let result = parser.parse_branches(base, compare).await?;
        println!("CRAI Summary: {} -> {}\n", base, compare);
        result
    };
    println!("Files changed: {}", diff_result.files.len());

    let total_chunks: usize = diff_result.files.iter().map(|f| f.chunks.len()).sum();
    println!("Total chunks: {}", total_chunks);

    if !cli.no_ai {
        let provider = AiProviderFactory::create(&config.ai)?;
        let filter = ChunkFilter::new(config.filters.clone())?;
        let orchestrator = ScoringOrchestrator::new(
            provider.clone(),
            filter,
            config.ai.concurrent_requests,
        );

        println!("\nRunning AI analysis...");

        let result = orchestrator
            .score_all(&diff_result.files, &ScoringContext::default(), |update: ScoringUpdate| {
                eprint!("\rScoring: {}/{}", update.progress.completed, update.progress.total);
            })
            .await?;

        eprintln!();

        println!("\nResults:");
        println!("  Reviewable chunks: {}", result.reviewable_count());
        println!("  Filtered chunks: {}", result.stats.filtered_chunks);
        println!("  Filtered lines: {} ({:.1}%)",
            result.stats.filtered_lines,
            result.stats.filter_percentage()
        );

        if let Some(avg) = result.average_score() {
            println!("  Average score: {:.2}", avg);
        }
        if let Some(max) = result.max_score() {
            println!("  Max score: {:.2}", max);
        }

        // Show high-score items
        let high_scores: Vec<_> = result
            .scores
            .iter()
            .filter(|s| s.score().map(|sc| sc >= 0.7).unwrap_or(false))
            .collect();

        if !high_scores.is_empty() {
            println!("\nHigh-concern items:");
            for score in high_scores.iter().take(10) {
                if let Some(resp) = &score.response {
                    let file = &diff_result.files[score.file_index];
                    println!(
                        "  [{:.0}%] {} - {} - {}",
                        resp.score * 100.0,
                        resp.classification,
                        file.path.display(),
                        resp.reasoning.chars().take(60).collect::<String>()
                    );
                }
            }
        }
    }

    Ok(())
}

async fn run_interactive(cli: &Cli, config: &Config) -> CraiResult<()> {
    let git = GitOperations::new(cli.repo.clone());
    git.verify_repository().await?;

    let parser = DiffParser::new(cli.repo.clone(), config.diff.context_lines);

    let diff_result = if cli.staged {
        parser.parse_staged().await?
    } else if cli.unstaged || (cli.base.is_none() && cli.compare.is_none()) {
        // Default to unstaged if no flags or branches specified
        parser.parse_unstaged().await?
    } else {
        let base = cli
            .base
            .as_deref()
            .unwrap_or(&config.general.default_base_branch);
        let compare = cli.compare.as_deref().unwrap_or("HEAD");

        git.verify_branch(base).await?;
        git.verify_branch(compare).await?;

        parser.parse_branches(base, compare).await?
    };

    if diff_result.files.is_empty() {
        println!(
            "No changes found between {} and {}",
            diff_result.base_branch, diff_result.compare_branch
        );
        return Ok(());
    }

    // Create app (terminal initialized later, after AI scoring)
    let mut app = App::new(config.clone(), diff_result);

    // Run AI scoring before entering TUI (show progress in terminal)
    // Terminal is NOT in raw mode here, so Ctrl+C works normally
    if !cli.no_ai {
        use std::io::Write;

        // Set up Ctrl+C handler
        let cancelled = Arc::new(AtomicBool::new(false));
        let cancelled_clone = cancelled.clone();
        let ctrlc_handler = tokio::spawn(async move {
            if tokio::signal::ctrl_c().await.is_ok() {
                cancelled_clone.store(true, Ordering::SeqCst);
                eprintln!("\n\nCancelled by user (Ctrl+C)");
            }
        });

        let provider = AiProviderFactory::create(&config.ai)?;
        let filter = ChunkFilter::new(config.filters.clone())?;

        // Clone what we need for scoring
        let files = app.diff_result.files.clone();
        let total_chunks: usize = files.iter().map(|f| f.chunks.len()).sum();

        println!("Analyzing {} files with {} chunks... (Ctrl+C to cancel)", files.len(), total_chunks);
        print!("  Applying heuristic filters... ");
        let _ = std::io::stdout().flush();

        let orchestrator = ScoringOrchestrator::new(
            provider.clone(),
            filter,
            config.ai.concurrent_requests,
        );

        // Run scoring with real-time progress and findings display
        let mut first_progress = true;
        let mut highlights_found = 0usize;
        let cancelled_check = cancelled.clone();
        let result = orchestrator
            .score_all(&files, &ScoringContext::default(), |update: ScoringUpdate| {
                // Check if cancelled
                if cancelled_check.load(Ordering::SeqCst) {
                    return;
                }

                if first_progress {
                    println!("done");
                    println!("  AI scoring chunks...\n");
                    first_progress = false;
                }

                let progress = &update.progress;

                // Show finding details if we have one
                if let Some(finding) = &update.finding {
                    // Clear the progress line and show finding
                    let status = if finding.is_filtered {
                        "\x1b[90m[filtered]\x1b[0m"
                    } else {
                        highlights_found += 1;
                        match finding.score {
                            s if s >= 0.7 => "\x1b[91m[review]\x1b[0m  ",
                            s if s >= 0.5 => "\x1b[93m[notable]\x1b[0m ",
                            _ => "\x1b[92m[routine]\x1b[0m ",
                        }
                    };

                    // Truncate reasoning to fit on one line (char-aware for UTF-8)
                    let reasoning = finding.reasoning.replace('\n', " ");
                    let max_reason_chars = 60;
                    let truncated_reason: String = if reasoning.chars().count() > max_reason_chars {
                        format!("{}...", reasoning.chars().take(max_reason_chars).collect::<String>())
                    } else {
                        reasoning
                    };

                    // Truncate file path if too long (char-aware for UTF-8)
                    let max_path_chars = 40;
                    let file_display = if finding.file_path.chars().count() > max_path_chars {
                        let skip = finding.file_path.chars().count() - max_path_chars + 3;
                        format!("...{}", finding.file_path.chars().skip(skip).collect::<String>())
                    } else {
                        finding.file_path.clone()
                    };

                    println!(
                        "  {} {:>3.0}% {} {}",
                        status,
                        finding.score * 100.0,
                        file_display,
                        truncated_reason
                    );
                }

                // Show progress bar
                let bar_width = 30;
                let filled = (progress.percentage() / 100.0 * bar_width as f64) as usize;
                let bar: String = "█".repeat(filled) + &"░".repeat(bar_width - filled);
                eprint!(
                    "\r  Progress: [{}] {}/{} ({:.0}%)   ",
                    bar,
                    progress.completed,
                    progress.total,
                    progress.percentage()
                );
                let _ = std::io::stderr().flush();
            })
            .await;

        // Cancel the Ctrl+C handler
        ctrlc_handler.abort();

        // Check if user cancelled
        if cancelled.load(Ordering::SeqCst) {
            return Ok(());
        }

        // Handle scoring result
        let result = result?;

        if first_progress {
            // No AI scoring was done (all filtered)
            println!("done (all chunks filtered)");
        } else {
            eprintln!(); // Clear progress line
            println!("\n  Scoring complete: {} highlights found", highlights_found);
        }

        println!(
            "  Result: {} reviewable | {} filtered ({:.1}%)",
            result.reviewable_count(),
            result.stats.filtered_lines,
            result.stats.filter_percentage()
        );

        app.set_scoring_result(result);

        // Generate AI summary
        print!("  Generating summary... ");
        let _ = std::io::stdout().flush();

        match provider
            .generate_summary(&files, &SummaryContext::default())
            .await
        {
            Ok(summary) => {
                println!("done");
                app.set_summary(summary);
            }
            Err(e) => {
                println!("failed: {}", e);
            }
        }
    }

    // Now initialize terminal for TUI (after AI scoring completes)
    let mut terminal = tui::init_terminal()?;

    // Event handler
    let events = EventHandler::new(100);

    // Main event loop
    loop {
        // Draw
        terminal.draw(|frame| {
            LayoutManager::render(frame, &app);
        })?;

        // Handle events
        match events.next()? {
            Event::Key(key) => {
                let action = Action::from_key(key);
                app.handle_action(action)?;
            }
            Event::Resize(_, _) => {
                // Clear and force full redraw on terminal resize
                terminal.clear()?;
            }
            Event::Tick => {
                // Could update progress here
            }
        }

        if app.should_quit {
            break;
        }
    }

    // Restore terminal
    tui::restore_terminal()?;

    Ok(())
}
