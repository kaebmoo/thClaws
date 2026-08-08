#!/usr/bin/env bash
# Remove bulky build outputs while keeping runnable thClaws binaries.
#
# Typical use after a debug GUI build:
#   scripts/build.sh --no-frontend
#   scripts/prune-build-artifacts.sh --prune-frontend
#
# The script keeps known executables in target/<profile>/ and removes the
# Cargo cache/output directories around them, so ./target/debug/thclaws can
# still run without rebuilding.

set -euo pipefail

PROFILE="debug"
PRUNE_FRONTEND=0
DRY_RUN=0
KEEP_BINS=("thclaws" "thclaws-cli" "thclaws-policy-tool" "catalogue-seed")

usage() {
    cat <<'EOF'
thClaws build artifact pruner

Usage:
  scripts/prune-build-artifacts.sh [options]

Options:
  --profile debug|release   Prune target/<profile> (default: debug)
  --keep-bin NAME           Preserve another top-level binary in target/<profile>
  --prune-frontend          Also remove frontend/node_modules
  --dry-run                 Print what would be removed, but do not delete
  -h, --help                Show this help

Examples:
  scripts/build.sh --no-frontend
  scripts/prune-build-artifacts.sh --prune-frontend

  scripts/build.sh --release --no-frontend
  scripts/prune-build-artifacts.sh --profile release --prune-frontend
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --profile)
            [[ $# -ge 2 ]] || { printf 'error: --profile needs a value\n' >&2; exit 2; }
            PROFILE="$2"
            shift 2
            ;;
        --keep-bin)
            [[ $# -ge 2 ]] || { printf 'error: --keep-bin needs a value\n' >&2; exit 2; }
            KEEP_BINS+=("$2")
            shift 2
            ;;
        --prune-frontend)
            PRUNE_FRONTEND=1
            shift
            ;;
        --dry-run)
            DRY_RUN=1
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            printf 'error: unknown arg: %s\n' "$1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

case "$PROFILE" in
    debug|release) ;;
    *)
        printf 'error: --profile must be debug or release\n' >&2
        exit 2
        ;;
esac

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
BIN_DIR="$ROOT_DIR/target/$PROFILE"

if [[ ! -d "$BIN_DIR" ]]; then
    printf 'error: %s does not exist; build first\n' "$BIN_DIR" >&2
    exit 1
fi

is_keep_bin() {
    local name="$1"
    local keep
    for keep in "${KEEP_BINS[@]}"; do
        [[ "$name" == "$keep" ]] && return 0
    done
    return 1
}

remove_path() {
    local path="$1"
    if [[ "$DRY_RUN" -eq 1 ]]; then
        printf 'would remove: %s\n' "$path"
    else
        rm -rf -- "$path"
        printf 'removed: %s\n' "$path"
    fi
}

preserved_count=0
for keep in "${KEEP_BINS[@]}"; do
    if [[ -x "$BIN_DIR/$keep" && ! -d "$BIN_DIR/$keep" ]]; then
        preserved_count=$((preserved_count + 1))
    fi
done

if [[ "$preserved_count" -eq 0 ]]; then
    printf 'error: no runnable binaries found in %s; refusing to prune\n' "$BIN_DIR" >&2
    exit 1
fi

printf 'pruning %s while preserving runnable binaries\n' "$BIN_DIR"

shopt -s nullglob dotglob
for entry in "$BIN_DIR"/*; do
    name="$(basename "$entry")"
    if is_keep_bin "$name" && [[ -x "$entry" && ! -d "$entry" ]]; then
        printf 'kept: %s\n' "$entry"
    else
        remove_path "$entry"
    fi
done
shopt -u nullglob dotglob

if [[ "$PRUNE_FRONTEND" -eq 1 && -d "$ROOT_DIR/frontend/node_modules" ]]; then
    remove_path "$ROOT_DIR/frontend/node_modules"
fi

if [[ "$DRY_RUN" -eq 1 ]]; then
    printf '\ncurrent size:\n'
else
    printf '\nremaining size:\n'
fi
du -sh "$ROOT_DIR" "$ROOT_DIR/target" "$BIN_DIR" 2>/dev/null || true
if [[ -d "$ROOT_DIR/frontend" ]]; then
    du -sh "$ROOT_DIR/frontend" 2>/dev/null || true
fi
