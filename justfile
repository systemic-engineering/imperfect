# imperfect — success | partial | failure

# Run all tests
test:
    nix develop -c cargo test

# Run tests with coverage report
coverage:
    nix develop -c cargo llvm-cov

# Coverage gate — fails if below threshold
pre-push:
    nix develop -c cargo llvm-cov

# Build workspace
build:
    nix develop -c cargo build

# Clippy lint
lint:
    nix develop -c cargo clippy -- -D warnings

# Format check
fmt-check:
    nix develop -c cargo fmt --check

# Format
fmt:
    nix develop -c cargo fmt

# Full check (what CI and pre-push hook run)
check: test lint pre-push
