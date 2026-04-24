#!/usr/bin/env bash
set -euo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd -- "$script_dir/.." && pwd)

plugin_out=$(
  nix --extra-experimental-features "nix-command flakes" \
    build "$repo_root#libnix-value-json" \
    --no-link \
    --print-out-paths
)

python3 "$script_dir/libnix_value_json.py" \
  "$plugin_out/lib/libnix-value-json.so" \
  --cases "$script_dir/libnix_value_json.csv"
