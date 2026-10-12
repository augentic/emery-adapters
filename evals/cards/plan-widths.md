# The plan widths

A round of the eval, run and read under [the protocol](README.md#protocol); [the README](README.md#rounds) indexes every round.

Measured 2026-10-08, with no turn spent, over every plan the runner had staged under `target/eval/` — 248 `run-N.plan.json` files over 29 cases: 209 from the seventeen dated cards of `2026-10-02T03-44-10Z` through `2026-10-04T07-05-39Z`, and 39 from the sixteen one-case directories the runner wrote before cards were dated. Each plan's slices were grouped into *waves* by the rule the engine's `Plan::waves` now applies and its `specify` and `build` envelopes report (the `waves` column the scorecard gained the same day): the first wave every slice whose `depends-on` is empty, each wave after it every slice whose dependencies are all in the waves before, in id order; a self-dependency or an id the plan does not hold is dropped, and a cycle would be one last wave. No plan held a cycle. The question the figures answer is the build architecture's: how much of a plan a concurrent build could run at once, where the serial build walks the waves flattened. *Serial ÷ waves* is the number of slices over the number of waves — the factor a build with no cap on width and slices of equal cost would shorten the critical path by — and *widest* is the cap at which that factor is reached.

| Case | Plans | Slices | Waves | Widest | Widest > 1 | Serial ÷ waves | Commonest shape |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `django-styleguide` | 8 | 10–12 (median 12) | 4–5 (median 4) | 4–6 (median 5) | 8/8 | 2.0–3.0 (median 3) | `1/5/3/3` ×2 of 7 |
| `nestjs-pair` | 9 | 9–19 (median 11) | 5–9 (median 7) | 3–7 (median 4) | 9/9 | 1.29–2.8 (median 1.83) | `1/1/1/6/1/1`, of 9 |
| `cross-cut` | 15 | 9–25 (median 10) | 5–11 (median 7) | 2–6 (median 2) | 15/15 | 1.43–2.78 (median 1.67) | `2/2/2/2/1/1` ×3 of 11 |
| `fastify-demo` | 6 | 10 | 3–6 (median 5) | 2–5 (median 3) | 6/6 | 1.67–3.33 (median 2) | `2/1/2/2/3` ×2 of 5 |
| `cli-jobs` | 5 | 6 | 4–5 (median 5) | 2–3 (median 2) | 5/5 | 1.2–1.5 (median 1.2) | `1/2/1/1/1` ×4 of 2 |
| `click-jobs` | 9 | 6 | 4–5 (median 5) | 2 | 9/9 | 1.2–1.5 (median 1.2) | `1/2/1/1/1` ×8 of 2 |
| `fastapi-template` | 9 | 5–6 (median 6) | 4–5 (median 4) | 2 | 9/9 | 1.2–1.5 (median 1.5) | `1/1/2/2` ×3 of 4 |
| `nestjs-boilerplate` | 6 | 6 | 3–5 (median 4) | 2–3 (median 2.5) | 6/6 | 1.2–2.0 (median 1.5) | `1/2/2/1` ×2 of 4 |
| `nestjs-docs` | 9 | 6–12 (median 6) | 3–7 (median 4) | 2–4 (median 3) | 9/9 | 1.5–2.0 (median 2) | `2/2/2` ×2 of 8 |
| `adr` | 12 | 1–6 (median 5) | 1–5 (median 5) | 1–2 (median 1) | 3/12 | 1.0–1.25 (median 1) | `1/1/1/1/1` ×8 of 4 |
| `cli-table` | 6 | 4 | 2–3 (median 3) | 2–3 (median 2) | 6/6 | 1.33–2.0 (median 1.33) | `1/2/1` ×4 of 2 |
| `express-orders` | 4 | 4 | 3 | 2 | 4/4 | 1.33 | `2/1/1` ×3 of 2 |
| `fastapi-routers` | 9 | 4 | 3 | 2 | 9/9 | 1.33 | `1/2/1` ×8 of 2 |
| `flask-blueprints` | 9 | 3–4 (median 4) | 2–3 (median 3) | 1–3 (median 2) | 8/9 | 1.0–2.0 (median 1.33) | `1/2/1` ×6 of 4 |
| `hono-mounted` | 4 | 4 | 3–4 (median 3.5) | 1–2 (median 1.5) | 2/4 | 1.0–1.33 (median 1.17) | `1/1/1/1` ×2 of 2 |
| `api-routes` | 7 | 3 | 2–3 (median 2) | 1–2 (median 2) | 5/7 | 1.0–1.5 (median 1.5) | `2/1` ×5 of 2 |
| `django-shop` | 9 | 3 | 3 | 1 | 0/9 | 1.0 | `1/1/1` ×9 |
| `money-library` | 9 | 3 | 2 | 2 | 9/9 | 1.5 | `1/2` ×9 |
| `narrative` | 11 | 3–9 (median 3) | 3–7 (median 3) | 1–2 (median 1) | 4/11 | 1.0–1.5 (median 1) | `1/1/1` ×7 of 5 |
| `stories` | 13 | 1–6 (median 3) | 1–2 (median 1) | 1–4 (median 3) | 12/13 | 1.0–3.0 (median 2.5) | `2` ×4 of 6 |
| `orders-triad` | 12 | 2–7 (median 2.5) | 2–6 (median 2.5) | 1–2 (median 1) | 2/12 | 1.0–1.33 (median 1) | `1/1` ×6 of 6 |
| `fastapi-orders` | 10 | 2 | 1–2 (median 2) | 1–2 (median 1) | 1/10 | 1.0–2.0 (median 1) | `1/1` ×9 of 2 |
| `orders-brief` | 3 | 1–3 (median 2) | 1–3 (median 2) | 1 | 0/3 | 1.0 | `1`, `1/1`, `1/1/1` |
| `orders-conflict` | 12 | 2–4 (median 2) | 2–4 (median 2) | 1 | 0/12 | 1.0 | `1/1` ×10 of 3 |
| `orders-divergence` | 12 | 2–4 (median 2) | 2–4 (median 2) | 1 | 0/12 | 1.0 | `1/1` ×9 of 3 |
| `r9k` | 3 | 2 | 2 | 1 | 0/3 | 1.0 | `1/1` ×3 |
| `orders` | 3 | 1 | 1 | 1 | 0/3 | 1.0 | `1` ×3 |
| `orders-doc` | 15 | 1–3 (median 1) | 1–3 (median 1) | 1 | 0/15 | 1.0 | `1` ×12 of 2 |
| `orders-spelled` | 9 | 1–2 (median 1) | 1–2 (median 1) | 1 | 0/9 | 1.0 | `1` ×6 of 2 |

A shape is each wave's width in build order, so `1/2/1/1/1` is a `start` slice alone, two slices ready after it, then a chain of three.

**Overall.** Slices 1–25 a plan, median 4, mean 4.85 (1 203 slices in all); waves 1–11, median 3, mean 3.31; widest 1–7, median 2, mean 1.94 (107 plans at 1, 94 at 2, 20 at 3, 15 at 4, 6 at 5, 5 at 6, 1 at 7). 216 plans declare a dependency somewhere; 24 are one slice; 83 are a pure chain (as many waves as slices); 8 of more than one slice declare no dependency at all. 141 of 248 plans are wider than one somewhere, 47 are three or wider, and 12 are wider than four. The first wave holds 40% of a plan's slices on average, median width 1 — the slicing brief leads with a `start` or bootstrap slice everything else waits on, and the width sits in the second and third waves. Serial ÷ waves is 1.0–3.33, median 1.2, mean 1.40 (1.44 over the 224 plans of more than one slice); under a width cap of 2 the mean is 1.26, of 3 1.34, of 4 1.38, and at 8 the full 1.40, since 636 of the 1 203 slices sit in a wave wider than one.

**By size.** The width is where the plans are. The 42 plans of nine slices or more (`cross-cut`, `django-styleguide`, `fastify-demo`, `narrative`, `nestjs-docs`, `nestjs-pair`) are 3–11 waves (median 6) and 2–7 wide (median 4), with serial ÷ waves 1.29–3.33 (median 1.87, mean 2.05; 1.53 under a cap of 2, 1.77 at 3, 1.91 at 4). The two real codebases of ten slices and more are the widest cases in the set: `django-styleguide` 4–6 wide on 8/8 plans, serial ÷ waves 2.0–3.0; `nestjs-pair` 3–7 wide on 9/9, 1.29–2.8. The authored cases of one to four slices are chains or singletons — every `orders-*` plan, `django-shop`, `r9k` — where a concurrent build would run nothing at once, and the six-slice adapter fixtures (`cli-jobs`, `click-jobs`, `fastapi-template`, `nestjs-boilerplate`) are 2 wide at one wave each, a 1.2–1.5 factor.

**What it says for phase 3.** At today's scale a concurrent build pays on the plans that take longest to build serially and on none of the small ones: the mean plan would shorten its critical path by 1.4×, the plans of nine slices or more by 2×, and the widest real codebase by 3×, while a plan of a chain is unchanged — so the serial order of the shipped build, the waves flattened, is the right default and a width cap is a per-run choice, not a plan property. A cap of 4 reaches 1.38 of the uncapped 1.40 overall and 1.91 of 2.05 on the large plans, and only 12 plans of 248 are ever wider; nothing in this set asks for more than four slices in flight. The data is the slicing brief's `depends-on` as the model wrote it over two days of cards, under `composer-2.5` on nearly every run, and the 8 plans of several slices with no dependency at all and the 83 pure chains are the two ends the brief's ordering rule can move between; a concurrent build's gain is bounded by how the slicing turn draws the edges, so the `waves` column is read beside `slices` on every card from here, and a slicing arm that widens or narrows the plans is graded on it. No code moved on this measurement; the script that read the plans lived outside the tree and is not kept.
