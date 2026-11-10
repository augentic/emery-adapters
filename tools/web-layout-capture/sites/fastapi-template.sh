# Experiment / non-production. The Full Stack FastAPI Template's frontend
# (React, Vite, TanStack Router) at a pinned commit, the one `backend/` the
# `fastapi-template` case reads: its login page, where `/` lands a visitor
# who has not signed in. The production build is served, so neither
# devtools panel paints into the page, and the page renders with no backend.
# The workspace's `bun.lock` pins the dependencies; bun 1.2 or later reads it.

ARCHIVE=https://github.com/fastapi/full-stack-fastapi-template/archive/cb740b656d7a0a6c5e12c7bf8e50343ec94ee9c7.tar.gz
ROUTE=/login
REQUIRES=(bun)

site_fetch() {
  curl -fsSL "$ARCHIVE" | tar -xz --strip-components=1 --exclude='*/backend'
}

site_install() {
  bun install --frozen-lockfile --ignore-scripts
}

site_build() {
  (cd frontend && bun run build)
}

site_serve() {
  cd frontend && exec node ../node_modules/vite/bin/vite.js preview --host 127.0.0.1 --port "$1" --strictPort
}
