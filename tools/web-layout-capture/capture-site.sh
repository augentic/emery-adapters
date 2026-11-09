#!/usr/bin/env bash
# Experiment / non-production. Fetches a site by name, serves it on
# 127.0.0.1, captures one page of it as rendered-layout facts, and stops it.
#
#   tools/web-layout-capture/capture-site.sh <site> <out> [capture options]
#
# <site> names a recipe under sites/: the archive the site's source is
# fetched from at a pinned commit, how it installs, builds, and serves, and
# the route captured. With no arguments the recipes are listed. Options after
# <out> (`--screenshots DIR`, a range) are `web-layout-capture capture`'s.
# The page is served at one port, `CAPTURE_SITE_PORT` or 4173, since the
# facts record the URL they were taken at. Every install runs with
# `--ignore-scripts`; the site's own code runs on this machine as it would
# served by hand.

set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)

if [ $# -lt 2 ]; then
  echo "usage: $0 <site> <out> [capture options]" >&2
  echo "sites:" >&2
  for recipe in "$here"/sites/*.sh; do
    echo "  $(basename "$recipe" .sh)" >&2
  done
  exit 2
fi

site=$1
out=$2
shift 2

recipe="$here/sites/$site.sh"
if [ ! -f "$recipe" ]; then
  echo "no site \`$site\` under $here/sites" >&2
  exit 2
fi
if [ ! -d "$here/node_modules/playwright" ]; then
  echo "the capture script's Playwright is not installed: npm ci --ignore-scripts --prefix $here" >&2
  exit 2
fi

# shellcheck source=/dev/null
. "$recipe"
for tool in curl node ${REQUIRES[@]+"${REQUIRES[@]}"}; do
  if ! command -v "$tool" > /dev/null; then
    echo "site \`$site\` needs \`$tool\` on PATH" >&2
    exit 2
  fi
done

mkdir -p "$(dirname "$out")"
out="$(cd "$(dirname "$out")" && pwd)/$(basename "$out")"
work=$(mktemp -d "${TMPDIR:-/tmp}/capture-site.XXXXXX")
server=
cleanup() {
  if [ -n "$server" ]; then
    kill "$server" 2> /dev/null || true
    wait "$server" 2> /dev/null || true
  fi
  rm -rf "$work"
}
trap cleanup EXIT

echo "capture-site: fetching $site" >&2
(cd "$work" && site_fetch)
echo "capture-site: installing" >&2
(cd "$work" && site_install)
echo "capture-site: building" >&2
(cd "$work" && site_build)

port=${CAPTURE_SITE_PORT:-4173}
url="http://127.0.0.1:$port$ROUTE"
if curl -s -o /dev/null "http://127.0.0.1:$port/"; then
  echo "capture-site: port $port is in use; free it or set CAPTURE_SITE_PORT" >&2
  exit 1
fi
(cd "$work" && site_serve "$port") > "$work/serve.log" 2>&1 &
server=$!

echo "capture-site: serving $url" >&2
for _ in $(seq 1 120); do
  if curl -sf "$url" > /dev/null; then
    break
  fi
  if ! kill -0 "$server" 2> /dev/null; then
    echo "capture-site: the server stopped before it answered:" >&2
    cat "$work/serve.log" >&2
    exit 1
  fi
  sleep 1
done
if ! curl -sf "$url" > /dev/null; then
  echo "capture-site: $url did not answer within two minutes:" >&2
  cat "$work/serve.log" >&2
  exit 1
fi

cargo run --manifest-path "$root/Cargo.toml" -q -p web-layout-capture -- \
  capture --url "$url" --out "$out" "$@"
