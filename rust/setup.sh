#!/usr/bin/env bash
#
# Everything the Rust side needs, installed once and never again.
#
#   ./rust/setup.sh
#
# Safe to run at any time: every step checks first and says what it found. Nothing here
# touches the system - rustup, the toolchain and trunk all live under ~/.cargo, and
# uninstalling is `rustup self uninstall`.

set -euo pipefail

say()  { printf '\033[1;34m==\033[0m %b\n' "$*"; }
have() { printf '   \033[1;32m✓\033[0m %b\n' "$*"; }
work() { printf '   \033[1;33m…\033[0m %b\n' "$*"; }
die()  { printf '\033[1;31m!!\033[0m %s\n' "$*" >&2; exit 1; }

# rustup puts things on PATH through ~/.cargo/env, which a non-login shell may not have read.
export PATH="$HOME/.cargo/bin:$PATH"

# --- the toolchain -------------------------------------------------------------------------
say "Rust toolchain"
if command -v rustup >/dev/null 2>&1; then
    have "$(rustup --version 2>/dev/null | head -1)"
else
    work "installing rustup (this fetches from https://sh.rustup.rs)"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path
    export PATH="$HOME/.cargo/bin:$PATH"
    command -v rustup >/dev/null 2>&1 || die "rustup did not install"
fi
have "rustc $(rustc --version | cut -d' ' -f2)"

# --- what the client compiles to -----------------------------------------------------------
say "WebAssembly target"
if rustup target list --installed | grep -qx wasm32-unknown-unknown; then
    have "wasm32-unknown-unknown"
else
    work "adding wasm32-unknown-unknown"
    rustup target add wasm32-unknown-unknown
fi

# --- what the editor uses --------------------------------------------------------------
say "components"
for component in clippy rustfmt rust-src; do
    if rustup component list --installed | grep -q "^$component"; then
        have "$component"
    else
        work "adding $component"
        rustup component add "$component"
    fi
done

# --- the client's build tool ---------------------------------------------------------------
# The version the Dockerfile pins, so a local build and the image's are the same build.
TRUNK_VERSION=$(grep -oP 'ARG TRUNK_VERSION=\K[0-9.]+' "$(dirname "$0")/../tourcalc.rust.docker" 2>/dev/null || echo "0.21.14")
say "trunk (the client's build tool)"
if command -v trunk >/dev/null 2>&1; then
    have "trunk $(trunk --version | awk '{print $2}') — the image builds with $TRUNK_VERSION"
else
    # The published binary, because `cargo install trunk` compiles it and that is minutes.
    ARCH=$(uname -m)
    case "$ARCH" in
        x86_64|aarch64) ;;
        *) die "no published trunk for $ARCH — try: cargo install trunk" ;;
    esac
    work "fetching trunk $TRUNK_VERSION"
    mkdir -p "$HOME/.cargo/bin"
    curl -fsSL "https://github.com/trunk-rs/trunk/releases/download/v${TRUNK_VERSION}/trunk-${ARCH}-unknown-linux-gnu.tar.gz" \
        | tar -xz -C "$HOME/.cargo/bin" trunk
    have "trunk $(trunk --version | awk '{print $2}')"
fi

# --- optional, and only for the database tests ---------------------------------------------
say "MongoDB (optional)"
if command -v mongod >/dev/null 2>&1 || [ -x /mnt/c/tmp/mongodb-7.0.14/bin/mongod ]; then
    have "a mongod is installed — the database tests can run against it"
else
    work "none found. The app runs without one (StorageType=InMemory) and the database"
    work "tests pass by doing nothing. See GUIDE.md if you want them to run for real."
fi

echo
say "ready. \033[1m./rust/build-and-run.sh\033[0m builds everything and starts the server."
