#!/usr/bin/env bash
set -euo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd -- "$script_dir/.." && pwd)
build_dir="$script_dir/builddir"

meson setup "$build_dir" "$repo_root/libnix-value-json" \
  -Db_coverage=true \
  -Dbuildtype=debug

meson compile -C "$build_dir"

python3 "$script_dir/libnix_value_json.py" \
  "$build_dir/libnix-value-json.so" \
  --cases "$script_dir/libnix_value_json.csv"

gcovr \
  --root "$repo_root" \
  --object-directory "$build_dir" \
  --filter "$repo_root/libnix-value-json/plugin.cpp" \
  --html-details "$build_dir/coverage.html"
