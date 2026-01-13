# Default recipe - show available commands
default:
    @just --list

# Build debug version
build:
    cargo build

# Build release version
release:
    cargo build --release

# Run tests
test:
    cargo test

# Run clippy lints
lint:
    cargo clippy

# Format code
fmt:
    cargo fmt

# Check formatting without modifying
fmt-check:
    cargo fmt -- --check

# Install to ~/.cargo/bin
install:
    cargo install --path .

# Install release build
install-release:
    cargo install --path . --release

# Run with example (unstaged changes)
run:
    cargo run

# Run with staged changes
run-staged:
    cargo run -- --staged

# Run doctor to check dependencies
doctor:
    cargo run -- doctor

# Clean build artifacts
clean:
    cargo clean

# Watch and rebuild on changes (requires cargo-watch)
watch:
    cargo watch -x build

# Run all checks (fmt, lint, test)
check: fmt-check lint test
