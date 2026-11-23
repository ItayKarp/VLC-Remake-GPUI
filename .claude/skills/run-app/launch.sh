#!/bin/sh
# Builds the app and starts it in the background with settings in a scratch folder, past
# onboarding, so the user's own settings, library and caches are untouched.
#
#   launch.sh CONFIG_DIR [LOG_FILE]
#
# Prints the app's process id. Stop it with `kill <pid>`.
set -eu
config=${1:?usage: launch.sh CONFIG_DIR [LOG_FILE]}
log=${2:-$config/app.log}
root=$(cd "$(dirname "$0")/../../.." && pwd)
cd "$root"
mkdir -p "$config"
[ -f "$config/local.toml" ] || printf 'onboarding_complete = true\n' > "$config/local.toml"
cargo build --features runtime_shaders >&2
swiftc -O .claude/skills/run-app/drive.swift -o target/drive >&2
RVLC_CONFIG_DIR="$config" ./target/debug/RedesignedVLC > "$log" 2>&1 &
echo $!
