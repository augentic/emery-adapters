# Omnia build — preparation

> Loaded with [`../build.md`](../build.md) for the preparation leg — the first leg of every omnia build. It owns exactly one job: produce a valid read-only checkout of [`augentic/omnia-exemplar`](https://github.com/augentic/omnia-exemplar) at `target/omnia-exemplar/` in the lent consumer workspace. Everything downstream depends on it: the adapter's deterministic scaffold prelude reads the template contract from the checkout, and the writer prompts read its compiling crates as the worked-code reference (see [`exemplar.md`](../../references/exemplar.md)).

## Checkout algorithm

Run inside the consumer workspace root. The checkout lives under `target/` (git-ignored, outside the cargo workspace) and tracks `main` unpinned — each build reads current `main`:

```bash
if [ -d target/omnia-exemplar/.git ]; then
  git -C target/omnia-exemplar fetch --depth 1 origin main \
    && git -C target/omnia-exemplar reset --hard origin/main
else
  git clone --depth 1 https://github.com/augentic/omnia-exemplar target/omnia-exemplar
fi
```

## Outcomes

- **Fresh clone or successful refresh** — answer `applicable: true` with a summary naming the checkout state (fresh or refreshed).
- **Refresh failed, previous checkout present** — proceed with the stale checkout; say so in the summary. The report leg records the staleness as a non-blocking finding.
- **No checkout obtainable** (clone failed, nothing on disk) — do not improvise a fallback: surface a stop hint per the build prompt's `## § Stop hint contract` in your summary (`failing-task`: the exemplar checkout step; `next-action`: retry after restoring access). The adapter's deterministic checkout validation then fails the build before generation.

## Constraints

- **Read-only.** Never edit the checkout, never add it to the workspace members, and never copy files wholesale into the consumer workspace.
- Write nothing else in this leg: no consumer code, no scaffold files — the adapter's deterministic prelude writes the tooling scaffold from the checkout after this leg completes.
- The compatibility contract (`exemplar.yaml`) and the navigation map live in [`exemplar.md`](../../references/exemplar.md); this prompt owns only the checkout.
- **Network / credentials.** The clone and refresh are agent-side `git` against GitHub. There is no host-side or baked-in exemplar copy: if the lent workspace cannot reach `https://github.com/augentic/omnia-exemplar`, the build fails closed at the scaffold prelude. Restore access (or keep a prior checkout under `target/omnia-exemplar/` for the stale-refresh path) before retrying.
