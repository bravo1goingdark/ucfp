#!/bin/bash
set -e

echo "=========================================="
echo "Running CI pipeline locally"
echo "=========================================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

run_step() {
    echo ""
    echo -e "${YELLOW}>>> $1${NC}"
    echo "=========================================="
    if eval "$2"; then
        echo -e "${GREEN}✓ $1 passed${NC}"
        return 0
    else
        echo -e "${RED}✗ $1 failed${NC}"
        return 1
    fi
}

FAILED=0

# Job 1: Format & Lint
echo ""
echo "=========================================="
echo "JOB 1: Format & Lint"
echo "=========================================="

if ! run_step "Check formatting" "cargo fmt --all -- --check"; then
    FAILED=1
    echo "Running cargo fmt to fix formatting..."
    cargo fmt --all
fi

if ! run_step "Run clippy" "cargo clippy --workspace --all-features -- -D warnings"; then
    FAILED=1
fi

# Job 2: Tests
echo ""
echo "=========================================="
echo "JOB 2: Tests"
echo "=========================================="

if ! run_step "Run tests" "cargo test --workspace --all-features"; then
    FAILED=1
fi

# Job 3: Build
echo ""
echo "=========================================="
echo "JOB 3: Build Check"
echo "=========================================="

if ! run_step "Build all targets" "cargo build --workspace --all-features"; then
    FAILED=1
fi

# Job 4: Documentation
echo ""
echo "=========================================="
echo "JOB 4: Documentation"
echo "=========================================="

if ! run_step "Check documentation" "RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --document-private-items"; then
    FAILED=1
fi

# Final summary
echo ""
echo "=========================================="
echo "CI Pipeline Summary"
echo "=========================================="
if [ $FAILED -eq 0 ]; then
    echo -e "${GREEN}✓ All jobs passed!${NC}"
    exit 0
else
    echo -e "${RED}✗ Some jobs failed${NC}"
    exit 1
fi
