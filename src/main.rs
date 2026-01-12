use clap::{Parser, Subcommand};
use crai::ai::provider::{AiProviderFactory, ScoringContext, SummaryContext};
use crai::ai::scoring::ScoringOrchestrator;
use crai::config::{self, Config};
use crai::diff::filter::ChunkFilter;
use crai::diff::git::GitOperations;
use crai::diff::parser::DiffParser;
use crai::error::CraiResult;
use crai::tui::event::{Action, Event, EventHandler};
use crai::tui::layout::LayoutManager;
use crai::tui::{self, App};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "crai")]
#[command(about = "AI-powered code review tool")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Path to config file
    #[arg(short, long, default_value = "crai.toml")]
    config: PathBuf,

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
        /// Output path for config
        #[arg(short, long, default_value = "crai.toml")]
        output: PathBuf,
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

    // Load config (use defaults if not found)
    let config = if cli.config.exists() {
        config::load_config(&cli.config)?
    } else {
        Config::default()
    };

    match cli.command {
        Some(Commands::Init { output }) => {
            config::create_default_config(&output)?;
            println!("Created config at {}", output.display());
            Ok(())
        }
        Some(Commands::Doctor) => run_doctor(&config).await,
        Some(Commands::Summary) => run_summary(&cli, &config).await,
        None => run_interactive(&cli, &config).await,
    }
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
    println!("  Config file: {}", if std::path::Path::new("crai.toml").exists() { "Found" } else { "Not found (using defaults)" });
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
            .score_all(&diff_result.files, &ScoringContext::default(), |progress| {
                eprint!("\rScoring: {}/{}", progress.completed, progress.total);
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

    // Initialize terminal
    let mut terminal = tui::init_terminal()?;

    // Create app
    let mut app = App::new(config.clone(), diff_result);

    // Run AI scoring before entering TUI (show progress in terminal)
    if !cli.no_ai {
        use std::io::Write;

        let provider = AiProviderFactory::create(&config.ai)?;
        let filter = ChunkFilter::new(config.filters.clone())?;

        // Clone what we need for scoring
        let files = app.diff_result.files.clone();
        let total_chunks: usize = files.iter().map(|f| f.chunks.len()).sum();

        println!("Analyzing {} files with {} chunks...", files.len(), total_chunks);
        print!("  Applying heuristic filters... ");
        let _ = std::io::stdout().flush();

        let orchestrator = ScoringOrchestrator::new(
            provider.clone(),
            filter,
            config.ai.concurrent_requests,
        );

        // Run scoring with progress display
        let mut first_progress = true;
        let result = orchestrator
            .score_all(&files, &ScoringContext::default(), |progress| {
                if first_progress {
                    println!("done");
                    print!("  AI scoring: ");
                    let _ = std::io::stdout().flush();
                    first_progress = false;
                }
                eprint!(
                    "\r  AI scoring: [{:>3}/{:>3}] {:.0}%   ",
                    progress.completed,
                    progress.total,
                    progress.percentage()
                );
                let _ = std::io::stderr().flush();
            })
            .await?;

        if first_progress {
            // No AI scoring was done (all filtered)
            println!("done (all chunks filtered)");
        } else {
            eprintln!("\r  AI scoring: [{}/{}] 100% - Done!      ",
                result.scores.iter().filter(|s| s.response.is_some()).count(),
                result.scores.len() - result.stats.filtered_chunks as usize);
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
                // Terminal will handle resize
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
