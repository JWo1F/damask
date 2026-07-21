#!/usr/bin/env bash
# Build the Damask website.
#
#   ./tools/build-site.sh              build into site/dist
#   ./tools/build-site.sh serve        …and serve it on http://localhost:8080
#   ./tools/build-site.sh watch        rebuild the CSS and the site on change
#
# Set BASE to deploy under a subpath — a GitHub project page lives at
# /<repo>/, so that build is `BASE=/damask ./tools/build-site.sh`.
#
# The CSS is compiled first because the generator copies `site/assets/` into the
# output verbatim: building the HTML against a stale stylesheet publishes the
# wrong one, silently and with a `dist/` that looks freshly made.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."
MODE="${1:-build}"
BASE="${BASE:-}"

./tools/get-tailwind.sh >/dev/null

css() {
  ./tools/tailwindcss -i site/ui/app.css -o site/assets/site.css --minify "$@"
}

case "${MODE}" in
  watch)
    # Tailwind watches its own inputs; the generator is re-run by hand, because
    # a file watcher that also rebuilds Rust is a job for `cargo watch` and not
    # for this script.
    css --watch
    ;;

  serve|build)
    echo "==> Compiling CSS"
    css
    echo "    $(wc -c < site/assets/site.css | tr -d ' ') bytes"

    echo "==> Rendering the site"
    cargo run --quiet -p damask-site -- --base "${BASE}"

    if [ "${MODE}" = "serve" ]; then
      echo "==> http://localhost:8080${BASE}/"
      # Python's server is not a production one, and does not need to be: it is
      # here so the clean URLs (`/book/slots/` → `book/slots/index.html`) resolve
      # the way they will once deployed, which opening a file:// URL does not.
      python3 -m http.server 8080 --directory site/dist
    fi
    ;;

  *)
    echo "usage: ./tools/build-site.sh [build|serve|watch]" >&2
    exit 1
    ;;
esac
