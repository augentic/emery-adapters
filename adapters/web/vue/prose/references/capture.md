# Capture

A turn's answer is scored from a capture of the page as it stands: the same measurement the original page's facts were taken with, run over your page in a headless browser. Run every step in `$WORKSPACE`.

1. **Install**, once per turn: `npm install --ignore-scripts`. The flag is required: no package's install script runs. The browser is not downloaded: the turn is lent the Playwright browsers already installed on the host, and the capture script finds them itself.
2. **Serve** the page in the background: `npm run dev > layout/dev.log 2>&1 &`, then wait until `curl -sf http://127.0.0.1:5173/ > /dev/null` succeeds. It serves `index.html` at `http://127.0.0.1:5173/`, the only URL any step opens, and reloads as `src/` changes. A compile error shows in `layout/dev.log` and in the capture's page errors.
3. **Capture**, after the last change to `src/`:

   ```sh
   node layout/capture.mjs --url http://127.0.0.1:5173/ --out layout/capture \
     --screenshots layout/capture-shots --stamp src \
     --min 320 --max 1920 --height 800 --height-min 400 --height-max 1440
   ```

   The numbers are the facts' own: `--min` and `--max` are `index.json` `range`, `--height` its `height`, `--height-min` and `--height-max` its `height_range`; take them from the index the turn reads. A capture that sweeps another range is refused. The capture records the browser it ran in, and one that ran in another browser than the original's facts did is refused. It takes about a minute and prints one line of counts; `page_errors` above 0 is a page that raised an error, and the capture is refused until it raises none.

`--stamp src` records the sha256 of every file under `src/` in the capture. The check re-hashes `src/` and refuses a capture of code that has changed since, naming each file; capture again after every change. `--screenshots` writes a full-page PNG per width to `layout/capture-shots/<width>.png`, the page as the browser drew it; read them to see what your page looks like.

The verification captures into `layout/verify/capture` with no screenshots; the rest is the same.

## The score

When your answer is checked, the capture is assembled into facts and compared with the original's at every width and height it sweeps. Each text run, image, box with a paint, and control of the original is paired with the render's by what it is and holds, and scored by where it lands and what it shows; the lowest score per kind, at any width, must reach 0.7. A failing score comes back as findings: each kind under the mark with the width it scores lowest at, the original items that hold at the fewest widths with why, the render's items that pair with nothing, and the overlay drawn at each width.

An overlay, `layout/review/overlay-<width>.svg` (`layout/verify/` in the verification), draws the original page in grey at the width the file names, every original item that did not hold in red, and every item of your page that paired with nothing in orange, each with why as its tooltip (`<title>` in the SVG). Read the overlays the findings name: they show where the page differs before the numbers say why.
