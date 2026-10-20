# Source before running cargo: `source desktop-rs/env.sh` (works from any worktree).
#
# Policy for this repo's AI sessions: nothing may be created outside the
# project directory. Cargo's registry/git cache and all temp files therefore
# live under desktop-rs/ (all git-ignored).
#
# - CARGO_HOME is shared by every worktree (lives in the *main* checkout).
# - target/ and .tmp/ are per-worktree so parallel agents never run each
#   other's binaries.
_GD_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
_GD_MAIN="$(cd "$_GD_ROOT" && cd "$(git rev-parse --path-format=absolute --git-common-dir)/.." && pwd)/desktop-rs"
export CARGO_HOME="$_GD_MAIN/.cargo-home"
export TMPDIR="$_GD_ROOT/.tmp"
export CARGO_TARGET_DIR="$_GD_ROOT/target"
export GD_CONFIG_DIR="$_GD_ROOT/.tmp/config"
mkdir -p "$CARGO_HOME" "$TMPDIR" "$GD_CONFIG_DIR"
