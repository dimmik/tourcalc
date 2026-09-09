#!/usr/bin/env bash
#
# Builds the Rust server, publishes the existing Blazor client next to it, and runs the two
# together so the whole app can be clicked through.
#
#   ./rust/build-and-run.sh              build everything and run
#   ./rust/build-and-run.sh --blazor     serve the Blazor client instead of the Rust one
#   ./rust/build-and-run.sh --fast       skip rebuilding the front end (reuse the last one)
#   ./rust/build-and-run.sh --port 5555  listen somewhere else
#
# The Rust client is the default. `--blazor` serves the one the C# server ships instead, on
# the same port and the same links - useful because this server is meant to be
# indistinguishable to it, so the two can be compared by restarting with the flag.

set -euo pipefail

cd "$(dirname "$0")/.."   # repository root
ROOT="$PWD"

PORT=5401
PUBLISH=1
RUST_UI=1
for arg in "$@"; do
    case "$arg" in
        --fast) PUBLISH=0 ;;
        --blazor) RUST_UI=0 ;;
        --rust-ui) RUST_UI=1 ;;   # still accepted, now the default
        --port) ;;                       # value is read below
        --port=*) PORT="${arg#*=}" ;;
        ''|*[!0-9]*) ;;                  # not a number: ignore
        *) PORT="$arg" ;;                # bare number after --port
    esac
done

say() { printf '\033[1;34m==\033[0m %b\n' "$*"; }
die() { printf '\033[1;31m!!\033[0m %s\n' "$*" >&2; exit 1; }

# --- the tools ---------------------------------------------------------------------------
# cargo lives in the home directory when rustup put it there, which a non-login shell may
# not have on PATH yet.
CARGO=$(command -v cargo || true)
[ -z "$CARGO" ] && [ -x "$HOME/.cargo/bin/cargo" ] && CARGO="$HOME/.cargo/bin/cargo"
[ -z "$CARGO" ] && die "cargo not found. Install with: curl https://sh.rustup.rs -sSf | sh"

# Under WSL the working .NET is usually the Windows one - and there may well be a Linux
# SDK on PATH that cannot actually run anything ("No frameworks were found"), so each
# candidate is tried rather than trusted.
usable_dotnet() {
    [ -x "$1" ] || command -v "$1" >/dev/null 2>&1 || return 1
    "$1" --list-runtimes 2>/dev/null | grep -q "Microsoft.NETCore.App"
}
DOTNET=""
for candidate in "$(command -v dotnet || true)" "/mnt/c/Program Files/dotnet/dotnet.exe"; do
    [ -n "$candidate" ] || continue
    if usable_dotnet "$candidate"; then DOTNET="$candidate"; break; fi
done

WWWROOT="$ROOT/rust/.run/wwwroot"

# --- the front end -------------------------------------------------------------------------
if [ "$RUST_UI" = 1 ]; then
    WWWROOT="$ROOT/rust/crates/tc-web/dist"
    if [ "$PUBLISH" = 1 ]; then
        TRUNK=$(command -v trunk || true)
        [ -z "$TRUNK" ] && [ -x "$HOME/.cargo/bin/trunk" ] && TRUNK="$HOME/.cargo/bin/trunk"
        [ -z "$TRUNK" ] && die "trunk not found. Install with: cargo install trunk"
        say "building the Rust client…"
        ( cd "$ROOT/rust/crates/tc-web" && "$TRUNK" build --release ) \
            || die "building the Rust client failed"
    fi
    [ -d "$WWWROOT" ] || die "no Rust client at $WWWROOT - run once without --fast"
    PUBLISH=0   # nothing else to build
fi

if [ "$PUBLISH" = 1 ]; then
    [ -z "$DOTNET" ] && die "no working .NET SDK found - needed to publish the client. Pass --fast to reuse the last build."
    say "publishing the Blazor client (a minute or so)…"
    OUT="$ROOT/rust/.run/publish"
    PROJ="$ROOT/TCBlazor/Server/TCBlazor.Server.csproj"
    # The Windows SDK needs Windows paths for both - a /mnt/c path starts with a slash, and
    # MSBuild reads anything starting with a slash as one of its own switches.
    case "$DOTNET" in
        /mnt/c/*)
            OUT_ARG=$(wslpath -w "$OUT")
            PROJ_ARG=$(wslpath -w "$PROJ")
            ;;
        *)
            OUT_ARG="$OUT"
            PROJ_ARG="$PROJ"
            ;;
    esac
    # The client build prints a few hundred lines of warnings that belong to the C# app and
    # not to anything here, so it is kept in a log and shown only when it fails.
    LOG="$ROOT/rust/.run/publish.log"
    mkdir -p "$(dirname "$LOG")"
    if ! "$DOTNET" publish "$PROJ_ARG" -c Release -o "$OUT_ARG" --nologo > "$LOG" 2>&1; then
        tail -30 "$LOG" >&2
        die "publishing the client failed - full log in $LOG"
    fi
    [ -d "$OUT/wwwroot" ] || die "the client published, but $OUT/wwwroot is not there"
    rm -rf "$WWWROOT"
    mkdir -p "$(dirname "$WWWROOT")"
    cp -r "$OUT/wwwroot" "$WWWROOT"
fi

[ -d "$WWWROOT" ] || die "no front end at $WWWROOT - run once without --fast"

# --- the server --------------------------------------------------------------------------
say "building the server…"
"$CARGO" build --release --manifest-path "$ROOT/rust/Cargo.toml" -p tc-server --quiet

BIN="$ROOT/rust/target/release/tc-server"
[ -x "$BIN" ] || die "the server did not build"

if command -v ss >/dev/null && ss -ltn 2>/dev/null | grep -q ":$PORT "; then
    die "port $PORT is busy. Pass another one: $0 --port 5555"
fi

# --- what to open ------------------------------------------------------------------------
SEED="$ROOT/TCBlazor/Server/inmemory-tours.json"
LINKS=$(python3 - "$SEED" "$PORT" <<'PY' 2>/dev/null || true
import json, sys, collections
seed, port = sys.argv[1], sys.argv[2]
tours = json.load(open(seed, encoding="utf-8-sig"))
# The seed is written in two spellings; read both.
def get(t, name):
    for k, v in t.items():
        if k.lower() == name.lower():
            return v
    return None
rows = []
for t in tours:
    tid, code = get(t, "id") or get(t, "guid"), get(t, "accessCodeMD5")
    name, sp = get(t, "name") or "", get(t, "spendings") or []
    if tid and code:
        rows.append((len(sp), tid, code, name))
rows.sort(reverse=True)
seen = set()
for _, tid, code, name in rows:
    if tid in seen:
        continue
    seen.add(tid)
    print(f"http://localhost:{port}/goto/{code}/{tid}\t{name}")
PY
)

echo
say "listening on \033[1mhttp://localhost:$PORT/\033[0m"
echo
echo "  A share link logs you in and opens the tour - no access code to type:"
echo
if [ -n "$LINKS" ]; then
    printf '%b\n' "$LINKS" | head -4 | while IFS=$'\t' read -r url name; do
        printf '    \033[1;36m%s\033[0m\n      %s\n' "$url" "$name"
    done
else
    echo "    http://localhost:$PORT/  (could not read the seed file for share links)"
fi
echo
echo "  Or open http://localhost:$PORT/ and type an access code. 'admin:master' logs in as"
echo "  administrator and shows every tour."
echo
if [ "$RUST_UI" = 1 ]; then
    printf "  Serving the \033[1mRust\033[0m client. It has the tour list and the Balance tab, and\n"
    echo "  computes the balances in the browser with the same tc-core the server uses."
    echo "  Run with --blazor for the original client on the same links."
else
    printf "  Serving the \033[1mBlazor\033[0m client - the one the C# server ships, unchanged.\n"
    echo "  Run without --blazor for the Rust one on the same links."
fi
echo
echo "  This server is read-only: adding and editing are phase 4."
echo
say "Ctrl+C to stop"
echo

MasterKey="${MasterKey:-master}" \
InMemoryFileName="$SEED" \
StaticFilesDir="$WWWROOT" \
Listen="0.0.0.0:$PORT" \
exec "$BIN"
