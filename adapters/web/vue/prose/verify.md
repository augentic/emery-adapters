# target.verify

Experiment / non-production. Capture the integrated page once and report whether its rendered layout holds to the original page's facts.

## Inputs

- `$WORKSPACE` — the integrated tree, in the [tree](references/tree.md)'s layout: the page under `src/`, and the original page's facts in `layout/original/`, which a build left there.

Nothing outside `$WORKSPACE` is reachable.

## The check

1. Install and serve as [capture](references/capture.md) says.
2. Capture into `layout/verify/capture`, stamped over `src`, with the sweep `layout/original/index.json` names:

   ```sh
   node layout/capture.mjs --url http://127.0.0.1:5173/ --out layout/verify/capture --stamp src \
     --min 320 --max 1920 --height 800 --height-min 400 --height-max 1440
   ```

   taking the numbers from `layout/original/index.json` `range`, `height`, and `height_range`.
3. Answer. The adapter scores the capture against `layout/original/` and writes `layout/verify/score.json` and an overlay per width, `layout/verify/overlay-<width>.svg`: the original in grey, every original item that did not hold in red, every item of the page that paired with nothing in orange.

This verification repairs nothing. Do not change `src/` or any other file: a change to `src/` after the capture is a capture of other code, and refused. The score is the verdict.

## Verdict

Answer with one JSON object:

- `passed` — whether the score passes: the lowest score per kind, at any width, reaches 0.7. Answer `true` first; when the score fails, the check says so and returns its findings.
- `failures` — empty when `passed` is true. When it is false, the score's findings exactly as the check returns them, each verbatim and in its order: each kind under the mark with the width it scores lowest at, the original items that hold at the fewest widths with why, the page's items that pair with nothing, and a line per overlay naming its path.

A failing verdict is the answer, not something to fix: report it.

## Worked example

A page whose capture scored above the pass mark at every width and height:

```json
{
  "passed": true,
  "failures": []
}
```
