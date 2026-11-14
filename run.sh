#!/usr/bin/env bash
# Runs the release build with secrets decrypted from .env by dotenvx (key in .env.keys).
set -euo pipefail
cd "$(dirname -- "$0")"
cargo build --release --quiet
exec dotenvx run -- ./target/release/dicebot
