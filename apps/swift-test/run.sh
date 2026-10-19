#!/bin/sh
set -eu

package_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
cache_dir="$package_dir/.build"
export CLANG_MODULE_CACHE_PATH="$cache_dir/clang-module-cache"
export SWIFTPM_MODULECACHE_OVERRIDE="$cache_dir/swiftpm-module-cache"

exec swift run \
    --package-path "$package_dir" \
    --disable-sandbox \
    --cache-path "$cache_dir/cache" \
    -Xswiftc -module-cache-path \
    -Xswiftc "$cache_dir/module-cache" \
    SwiftTest "$@"
