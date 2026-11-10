#!/usr/bin/env node
// Experiment / non-production. Captures one served page for the rendered-layout facts.
//
//   node capture.mjs --url URL --out DIR [--screenshots DIR] [--stamp DIR]
//     [--min 320] [--max 1920] [--height 800] [--height-min 400] [--height-max 1440]
//
// One page load, resized; the browser is the oracle. Widths are bisected at one height, then
// heights at each sweep width: a segment is accepted when its midpoint, 1/3, 2/3 and one seeded
// random point lie on the line through its ends. A second pass at every segment end records
// computed styles, attributes, pseudo-elements, sizing values and the accessibility tree as
// deltas. Writes DIR/meta.json, DIR/w/<width>.json, DIR/h/<width>/<height>.json, DIR/ax/,
// DIR/assets/; `web-rendered-layout` builds the facts from them.
//
// --screenshots DIR: a full-page PNG per range end and sweep width, each from a fresh load.
// --stamp DIR: records the sha256 of every file under DIR in meta.json `source`, so a reader can
// tell the capture is of the current code.
//
// The browser is Playwright's Chromium unless CAPTURE_BROWSER_CHANNEL names a channel ("chrome"),
// and meta.json `browser` records which and its version. It always runs as a single process,
// launched afresh for every load: a sandboxed worker may not register the Mach service that
// Chromium's child processes rendezvous through, a single-process browser does not survive a
// second context, and captures in and out of the sandbox must match.
// Kept to driving the browser: everything after the raw capture is Rust.

import crypto from "node:crypto";
import fs from "node:fs/promises";
import path from "node:path";
import { parseArgs } from "node:util";

// A sandboxed worker's HOME is not the operator's, so Playwright does not find its own browser
// cache there; the worker is lent each platform's cache under its own name instead.
const LENT_CACHE = { darwin: "PLAYWRIGHT_CACHE_MACOS", linux: "PLAYWRIGHT_CACHE_LINUX" }[process.platform];
if (!process.env.PLAYWRIGHT_BROWSERS_PATH && LENT_CACHE && process.env[LENT_CACHE]) {
  process.env.PLAYWRIGHT_BROWSERS_PATH = process.env[LENT_CACHE];
}
const { chromium } = await import("playwright");

const DEFAULTS = {
  min: 320, max: 1920, height: 800, tolerance: 1, seed: 7, maxCaptures: 1500, maxAssets: 300,
  heightMin: 400, heightMax: 1440,
  heightWidths: [360, 414, 480, 481, 512, 700, 767, 768, 769, 900, 1024, 1025, 1100, 1440, 1700],
};

// Everything that makes two captures of the same page differ is pinned here.
const CONTEXT = { deviceScaleFactor: 1, locale: "en-US", timezoneId: "UTC", colorScheme: "light", reducedMotion: "reduce" };
// `Date.now()` inside the page.
const FIXED_TIME = new Date("2026-01-01T00:00:00Z");
const FREEZE_ID = "__capture_freeze__";
const FREEZE_CSS =
  "*,*::before,*::after{animation:none!important;transition:none!important;caret-color:transparent!important;scroll-behavior:auto!important}";
const SEED_SCRIPT = `(() => { let s = 0x2f6b1a3d; Math.random = function () { s |= 0; s = (s + 0x6d2b79f5) | 0; let t = Math.imul(s ^ (s >>> 15), 1 | s); t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t; return ((t ^ (t >>> 14)) >>> 0) / 4294967296; }; })();`;

// Sizing properties also read through `computedStyleMap()`, which keeps `%`, `auto` and `calc()`.
const SIZING_PROPS = [
  "width", "height", "min-width", "min-height", "max-width", "max-height",
  "margin-top", "margin-right", "margin-bottom", "margin-left",
  "padding-top", "padding-right", "padding-bottom", "padding-left",
  "top", "right", "bottom", "left", "flex-basis", "row-gap", "column-gap",
  "grid-template-columns", "grid-template-rows", "font-size", "line-height",
];

// ---- in the page ----

function initInPage() {
  const SKIP = new Set(["SCRIPT", "STYLE", "LINK", "META", "NOSCRIPT", "TEMPLATE", "TITLE", "HEAD", "BASE"]);
  const cssPath = (el) => {
    const parts = [];
    for (let e = el; e && e.nodeType === 1; e = e.parentElement) {
      const tag = e.tagName.toLowerCase();
      if (!e.parentElement) { parts.unshift(tag); break; }
      const sib = [...e.parentElement.children].filter((s) => s.tagName === e.tagName);
      parts.unshift(sib.length > 1 ? `${tag}:nth-of-type(${sib.indexOf(e) + 1})` : tag);
    }
    return parts.join(" > ");
  };
  const els = [];
  for (const el of [document.documentElement, ...document.documentElement.querySelectorAll("*")]) {
    if (SKIP.has(el.tagName) || el.closest("head") || el.id === "__capture_freeze__") continue;
    const svg = el.parentElement?.closest("svg");
    if (svg) continue;
    els.push(el);
  }
  const index = new Map(els.map((e, i) => [e, i]));
  const texts = [];
  const elements = els.map((el) => {
    let p = el.parentElement, parent = -1;
    while (p && parent < 0) { parent = index.get(p) ?? -1; p = p.parentElement; }
    const tag = el.tagName.toLowerCase();
    return { p: cssPath(el), tag, parent, kind: tag === "svg" ? "svg" : tag === "img" ? "img" : "box", n: el.getAttribute("data-n") };
  });
  const textMeta = [];
  const seqs = els.map(() => []);
  els.forEach((el, i) => {
    if (elements[i].kind !== "box") return;
    let k = 0;
    const seq = seqs[i];
    for (const c of el.childNodes) {
      if (c.nodeType === 1) { const j = index.get(c); if (j !== undefined) seq.push(j); continue; }
      if (c.nodeType !== 3) continue;
      if (!c.data.trim()) { if (c.data.length && seq[seq.length - 1] !== " ") seq.push(" "); continue; }
      texts.push(c);
      textMeta.push({ p: `${elements[i].p} > #text:${++k}`, parent: i, text: c.data });
      seq.push(`t${texts.length - 1}`);
    }
  });
  elements.forEach((e, i) => { e.seq = seqs[i]; if (e.tag === "br") e.kind = "br"; });
  window.__rs = { els, texts };
  return { elements, texts: textMeta };
}

function measureInPage() {
  const { els, texts } = window.__rs;
  const r2 = (n) => Math.round(n * 100) / 100;
  const b = [];
  for (const el of els) {
    const r = el.getBoundingClientRect();
    const shown = getComputedStyle(el).display !== "none" && (r.width > 0 || r.height > 0) ? 1 : 0;
    b.push(r2(r.x + scrollX), r2(r.y + scrollY), r2(r.width), r2(r.height), shown);
  }
  const range = document.createRange();
  const charY = (t, i) => {
    for (let j = i; j < t.data.length; j++) {
      range.setStart(t, j); range.setEnd(t, j + 1);
      const rs = [...range.getClientRects()].filter((q) => q.width > 0);
      if (rs.length) return rs[0].top + rs[0].height / 2;
    }
    return Infinity;
  };
  const l = texts.map((t) => {
    range.selectNodeContents(t);
    const rs = [...range.getClientRects()].filter((q) => q.width > 0 && q.height > 0).sort((p, q) => p.top - q.top || p.left - q.left);
    if (!rs.length) return 0;
    const lines = [];
    for (const q of rs) {
      const last = lines[lines.length - 1];
      if (last && q.top + q.height / 2 < last.btm && q.top + q.height / 2 > last.y) { last.x = Math.min(last.x, q.left); last.r = Math.max(last.r, q.right); last.btm = Math.max(last.btm, q.bottom); }
      else lines.push({ x: q.left, y: q.top, r: q.right, btm: q.bottom });
    }
    const out = [lines.length];
    for (const q of lines) out.push(r2(q.x + scrollX), r2(q.y + scrollY), r2(q.r - q.x), r2(q.btm - q.y));
    for (let k = 1; k < lines.length; k++) {
      const bound = (lines[k - 1].btm + lines[k].y) / 2 - scrollY + scrollY;
      let lo = 0, hi = t.data.length;
      while (lo < hi) { const m = (lo + hi) >> 1; if (charY(t, m) > bound) hi = m; else lo = m + 1; }
      out.push(lo);
    }
    return out;
  });
  return { b, l };
}

function sizingInPage(props) {
  const { els } = window.__rs;
  return els.map((el) => { const m = el.computedStyleMap(); return props.map((p) => m.get(p)?.toString() ?? ""); });
}

function extrasInPage(idx) {
  const { els } = window.__rs;
  return idx.map((i) => {
    const el = els[i];
    if (el.tagName.toLowerCase() === "svg") return { svg: el.outerHTML.length < 200000 ? el.outerHTML : null };
    if (el instanceof HTMLImageElement) return { src: el.currentSrc || el.src, alt: el.alt };
    if (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement) return { placeholder: el.placeholder || null, value: el.type === "password" ? null : el.value || null, type: el.type };
    return {};
  });
}

// Every @font-face rule of the page with each descriptor as the browser parsed it; `texts` are
// cross-origin sheets fetched outside the page, parsed here by the browser's own parser.
function facesInPage(texts) {
  const faces = [];
  const cross = [];
  const sheet = (sh, base) => {
    let rules;
    try { rules = sh.cssRules; } catch { if (sh.href) cross.push(sh.href); return; }
    const walk = (rs) => {
      for (const r of [...rs]) {
        if (r instanceof CSSFontFaceRule) faces.push({ base, d: Object.fromEntries([...r.style].map((p) => [p, r.style.getPropertyValue(p)])) });
        else if (r instanceof CSSImportRule) { if (r.styleSheet) sheet(r.styleSheet, r.styleSheet.href ?? base); }
        else if ("cssRules" in r) walk(r.cssRules);
      }
    };
    walk(rules);
  };
  for (const sh of [...document.styleSheets, ...document.adoptedStyleSheets]) sheet(sh, sh.href || location.href);
  for (const { href, text } of texts) {
    const sh = new CSSStyleSheet();
    sh.replaceSync(text);
    sheet(sh, href);
  }
  return { faces, cross };
}

function metricsInPage(fonts) {
  const c = document.createElement("canvas").getContext("2d");
  return fonts.map(({ key, font }) => { c.font = font; const m = c.measureText("Hg"); return { key, ascent: m.fontBoundingBoxAscent / 100, descent: m.fontBoundingBoxDescent / 100 }; });
}

// ---- the search ----

// Seeded uniform [0, 1) (mulberry32).
function rng(seed) {
  let s = seed >>> 0;
  return () => {
    s = (s + 0x6d2b79f5) | 0;
    let t = Math.imul(s ^ (s >>> 15), 1 | s);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function onLine(lo, hi, mid, a, b, m, tol) {
  const t = (m - a) / (b - a);
  for (let i = 0; i < mid.b.length; i += 5) {
    const s = mid.b[i + 4];
    if (lo.b[i + 4] !== s || hi.b[i + 4] !== s) return false;
    if (!s) continue;
    for (let k = 0; k < 4; k++) if (Math.abs(lo.b[i + k] + (hi.b[i + k] - lo.b[i + k]) * t - mid.b[i + k]) > tol) return false;
  }
  for (let j = 0; j < mid.l.length; j++) {
    const n = (x) => (x ? x[0] : 0);
    if (n(lo.l[j]) !== n(mid.l[j]) || n(hi.l[j]) !== n(mid.l[j])) return false;
  }
  return true;
}

const twoFrames = () => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(() => { scrollTo(0, 0); r(); })));

async function measureAt(page, w, h) {
  await page.setViewportSize({ width: w, height: h });
  await page.evaluate(twoFrames);
  return page.evaluate(measureInPage);
}

async function bisect(min, max, measure, o) {
  const cache = new Map();
  let capped = false;
  const at = async (x) => { let m = cache.get(x); if (!m) { m = await measure(x); cache.set(x, m); } return m; };
  const random = rng(o.seed);
  const segments = [];
  const checks = [];
  const split = async (a, b) => {
    if (b - a <= 1) { segments.push([a, b]); return; }
    if (cache.size >= o.maxCaptures) { capped = true; segments.push([a, b]); return; }
    const probes = [["mid", Math.floor((a + b) / 2)]];
    if (b - a >= 6) probes.push(["third", a + Math.round((b - a) / 3)], ["two-thirds", a + Math.round((2 * (b - a)) / 3)]);
    if (b - a > 2) {
      let c = a + 1 + Math.floor(random() * (b - a - 1));
      while (probes.some((p) => p[1] === c) && c + 1 < b) c++;
      if (!probes.some((p) => p[1] === c)) probes.push(["random", c]);
    }
    for (const [kind, c] of probes) {
      const ok = onLine(await at(a), await at(b), await at(c), a, b, c, o.tolerance);
      checks.push({ segment: [a, b], kind, at: c, ok });
      if (!ok) { await split(a, c); await split(c, b); return; }
    }
    segments.push([a, b]);
  };
  await split(min, max);
  segments.sort((p, q) => p[0] - q[0]);
  const ends = [...new Set(segments.flat())].sort((p, q) => p - q);
  return { cache, segments, checks, capped, ends };
}

function delta(names, prev, vals) {
  const d = {};
  vals.forEach((v, k) => { if (!prev || prev[k] !== v) d[names[k]] = v; });
  return Object.keys(d).length ? d : undefined;
}

function deltaObj(prev, cur) {
  const d = {};
  for (const [k, v] of Object.entries(cur)) if (!prev || prev[k] !== v) d[k] = v;
  for (const k of Object.keys(prev ?? {})) if (!(k in cur)) d[k] = null;
  return Object.keys(d).length ? d : undefined;
}

const newState = () => ({ styles: new Map(), attrs: new Map(), pseudo: new Map(), sizing: new Map(), ax: "" });
const cloneState = (s) => ({ styles: new Map(s.styles), attrs: new Map(s.attrs), pseudo: new Map(s.pseudo), sizing: new Map(s.sizing), ax: s.ax });

async function backendIds(cdp, n) {
  const { result } = await cdp.send("Runtime.evaluate", { expression: "window.__rs.els" });
  const { result: items } = await cdp.send("Runtime.getProperties", { objectId: result.objectId, ownProperties: true });
  const ids = new Array(n).fill(-1);
  for (const it of items) {
    const i = Number(it.name);
    if (!Number.isInteger(i) || !it.value?.objectId) continue;
    ids[i] = (await cdp.send("DOM.describeNode", { objectId: it.value.objectId })).node.backendNodeId;
  }
  return ids;
}

async function settle(page) {
  await page.waitForLoadState("networkidle", { timeout: 15000 }).catch(() => console.error("capture: networkidle timeout"));
  await page.evaluate(() => Promise.race([document.fonts.ready.then(() => true), new Promise((r) => setTimeout(() => r(false), 10000))]));
}

const CHANNEL = process.env.CAPTURE_BROWSER_CHANNEL || undefined;

async function openSettled(url, viewport) {
  const browser = await chromium.launch({ channel: CHANNEL, headless: true, args: ["--single-process"] });
  const context = await browser.newContext({ ...CONTEXT, viewport });
  await context.addInitScript(SEED_SCRIPT);
  const page = await context.newPage();
  await page.clock.setFixedTime(FIXED_TIME);
  const errors = [];
  page.on("pageerror", (e) => errors.push(String(e).slice(0, 300)));
  await page.goto(url, { waitUntil: "load" });
  await settle(page);
  await page.addStyleTag({ content: FREEZE_CSS }).then((h) => h.evaluate((el, id) => { el.id = id; }, FREEZE_ID));
  await page.evaluate(async () => {
    for (let y = 0; y < document.documentElement.scrollHeight; y += innerHeight) { scrollTo(0, y); await new Promise((r) => setTimeout(r, 100)); }
    scrollTo(0, 0);
  });
  await settle(page);
  await page.waitForTimeout(300);
  return { browser, page, errors };
}

async function search(url, out, o) {
  const t0 = Date.now();
  const { browser, page, errors } = await openSettled(url, { width: o.max, height: o.height });
  await fs.rm(out, { recursive: true, force: true });
  await fs.mkdir(path.join(out, "w"), { recursive: true });
  await fs.mkdir(path.join(out, "assets"), { recursive: true });
  await fs.mkdir(path.join(out, "ax"), { recursive: true });
  try {
    const dom = await page.evaluate(initInPage);
    const { cache, segments, checks, capped, ends } = await bisect(o.min, o.max, (w) => measureAt(page, w, o.height), o);
    const searchMs = Date.now() - t0;

    const cdp = await page.context().newCDPSession(page);
    await cdp.send("DOM.enable"); await cdp.send("CSS.enable"); await cdp.send("Accessibility.enable");
    await cdp.send("DOM.getDocument", { depth: 0 });
    // Custom properties are left out: DOMSnapshot rejects them, and their effect is in the
    // computed value of every property that uses them.
    const props = await page.evaluate(() => [...getComputedStyle(document.documentElement)].filter((p) => !p.startsWith("--")));
    const fi = props.indexOf("font-family"), wi = props.indexOf("font-weight"), si = props.indexOf("font-style"), ci = props.indexOf("content");
    const backend = await backendIds(cdp, dom.elements.length);
    const extras = {};
    const tokens = {};
    const textParents = new Set(dom.texts.map((t) => t.parent));
    const token = async (vals, backendNodeId) => {
      const key = `${vals[fi]}|${vals[wi]}|${vals[si]}`;
      if (tokens[key]) return;
      let platform = [];
      try {
        const { nodeIds } = await cdp.send("DOM.pushNodesByBackendIdsToFrontend", { backendNodeIds: [backendNodeId] });
        if (nodeIds[0]) platform = (await cdp.send("CSS.getPlatformFontsForNode", { nodeId: nodeIds[0] })).fonts;
      } catch { /* recorded as empty */ }
      tokens[key] = { family: vals[fi], weight: vals[wi], style: vals[si], platform };
    };
    const factsAt = async (w, h, m, st) => {
      await page.setViewportSize({ width: w, height: h });
      await page.evaluate(twoFrames);
      const snap = await cdp.send("DOMSnapshot.captureSnapshot", { computedStyles: props });
      const str = (k) => (k < 0 ? "" : snap.strings[k]);
      const { nodes, layout } = snap.documents[0];
      const layoutOf = new Map(layout.nodeIndex.map((n, k) => [n, k]));
      const nodeOf = new Map(nodes.backendNodeId.map((b, n) => [b, n]));
      const indexOf = new Map();
      backend.forEach((b, i) => { const n = nodeOf.get(b); if (n !== undefined) indexOf.set(n, i); });
      const styleOf = (n) => { const k = layoutOf.get(n); return k === undefined ? undefined : layout.styles[k].map(str); };
      const s = {}, a = {};
      for (const [n, i] of indexOf) {
        const vals = styleOf(n);
        if (vals) { const d = delta(props, st.styles.get(i), vals); if (d) s[i] = d; st.styles.set(i, vals); }
        const at = nodes.attributes[n].map(str);
        const attrs = Object.fromEntries(at.flatMap((x, k) => (k % 2 ? [] : [[x, at[k + 1]]])));
        const da = deltaObj(st.attrs.get(i), attrs); if (da) a[i] = da; st.attrs.set(i, attrs);
        if (vals && m.b[i * 5 + 4] && textParents.has(i)) await token(vals, nodes.backendNodeId[n]);
      }
      const ps = {};
      const pt = nodes.pseudoType;
      for (const [k, n] of (pt?.index ?? []).entries()) {
        const type = str(pt.value[k]);
        const i = indexOf.get(nodes.parentIndex[n]);
        if (i === undefined || !["before", "after", "marker"].includes(type)) continue;
        const key = `${i}::${type}`, li = layoutOf.get(n), vals = styleOf(n);
        const e = { b: li === undefined ? null : layout.bounds[li].map((x) => Math.round(x * 100) / 100) };
        if (vals) {
          const d = delta(props, st.pseudo.get(key), vals); if (d) e.s = d; st.pseudo.set(key, vals);
          if (!["none", "normal"].includes(vals[ci])) await token(vals, nodes.backendNodeId[n]);
        }
        ps[key] = e;
      }
      const mv = {};
      (await page.evaluate(sizingInPage, SIZING_PROPS)).forEach((vals, i) => { const d = delta(SIZING_PROPS, st.sizing.get(i), vals); if (d) mv[i] = d; st.sizing.set(i, vals); });
      // CDP renumbers AX nodes on every call: ids become `n<element index>`, else
      // `b<backendDOMNodeId>`, else `<parent id>/<child position>`. InlineTextBox nodes are the
      // text's line boxes (already in `l`): left out.
      const axAll = (await cdp.send("Accessibility.getFullAXTree", {})).nodes;
      const inline = new Set(axAll.filter((x) => x.role?.value === "InlineTextBox").map((x) => x.nodeId));
      const axNodes = axAll.filter((x) => !inline.has(x.nodeId)).map((x) => (x.childIds ? { ...x, childIds: x.childIds.filter((c) => !inline.has(c)) } : x));
      const byAx = new Map(axNodes.map((x) => [x.nodeId, x]));
      const axId0 = new Map();
      const axName = (id) => {
        let v = axId0.get(id);
        if (v !== undefined) return v;
        const x = byAx.get(id), b = x.backendDOMNodeId, n = b === undefined ? undefined : nodeOf.get(b), i = n === undefined ? undefined : indexOf.get(n);
        const p = x.parentId ? byAx.get(x.parentId) : undefined;
        v = i !== undefined ? `n${i}` : b !== undefined ? `b${b}` : p ? `${axName(p.nodeId)}/${p.childIds.indexOf(id)}` : `x${id}`;
        axId0.set(id, v);
        return v;
      };
      for (const x of axNodes) axName(x.nodeId);
      const ax = JSON.stringify(axNodes.map(({ nodeId, parentId, childIds, backendDOMNodeId: _b, frameId: _f, ...x }) => ({
        id: axId0.get(nodeId), ...(parentId ? { parent: axId0.get(parentId) } : {}), ...(childIds?.length ? { children: childIds.map((c) => axId0.get(c)) } : {}), ...x,
      })).sort((p, q) => (p.id < q.id ? -1 : p.id > q.id ? 1 : 0)));
      const axId = crypto.createHash("sha256").update(ax).digest("hex").slice(0, 16);
      if (axId !== st.ax) { await fs.writeFile(path.join(out, "ax", `${axId}.json`), ax + "\n"); st.ax = axId; }
      const fresh = dom.elements.map((_, i) => i).filter((i) => m.b[i * 5 + 4] && !(i in extras) && (dom.elements[i].kind !== "box" || ["input", "textarea"].includes(dom.elements[i].tag)));
      if (fresh.length) (await page.evaluate(extrasInPage, fresh)).forEach((x, k) => { extras[fresh[k]] = x; });
      return { s, a, ps, m: mv, ax: axId };
    };
    // w/ holds the segment ends plus the sweep widths; each height sweep is a delta chain
    // starting from the facts at its width.
    const heightWidths = o.heightWidths.filter((x) => x >= o.min && x <= o.max);
    const wFiles = [...new Set([...ends, ...heightWidths])].sort((p, q) => p - q);
    const wState = newState();
    const base = new Map();
    for (const w of wFiles) {
      const m = cache.get(w) ?? (await measureAt(page, w, o.height));
      await fs.writeFile(path.join(out, "w", `${w}.json`), JSON.stringify({ w, b: m.b, l: m.l, ...(await factsAt(w, o.height, m, wState)) }) + "\n");
      if (heightWidths.includes(w)) base.set(w, cloneState(wState));
    }
    const pass2Ms = Date.now() - t0 - searchMs;

    const sweeps = {};
    for (const w of heightWidths) {
      const r = await bisect(o.heightMin, o.heightMax, (h) => measureAt(page, w, h), o);
      const dir = path.join(out, "h", String(w));
      await fs.mkdir(dir, { recursive: true });
      const hState = base.get(w);
      for (const h of r.ends) {
        const m = r.cache.get(h);
        await fs.writeFile(path.join(dir, `${h}.json`), JSON.stringify({ w, h, b: m.b, l: m.l, ...(await factsAt(w, h, m, hState)) }) + "\n");
      }
      sweeps[w] = { captures: r.cache.size, capped: r.capped, segments: r.segments, ends: r.ends, checks: r.checks.length };
    }
    const heightMs = Date.now() - t0 - searchMs - pass2Ms;

    const sheets = [];
    for (const href of (await page.evaluate(facesInPage, [])).cross) {
      try { sheets.push({ href, text: await (await page.context().request.get(href)).text() }); } catch { /* unreadable sheet: its faces stay unknown */ }
    }
    const { faces } = await page.evaluate(facesInPage, sheets);
    const assets = {};
    const fetchAsset = async (url) => {
      if (assets[url] || Object.keys(assets).length >= o.maxAssets || !/^https?:|^data:/.test(url)) return assets[url];
      try {
        let buf, type = "";
        if (url.startsWith("data:")) {
          const mm = /^data:([^;,]*)(;base64)?,(.*)$/s.exec(url);
          if (!mm) return undefined;
          type = mm[1]; buf = mm[2] ? Buffer.from(mm[3], "base64") : Buffer.from(decodeURIComponent(mm[3]));
        } else {
          const res = await page.context().request.get(url, { timeout: 15000 });
          if (!res.ok()) return undefined;
          buf = await res.body(); type = res.headers()["content-type"] ?? "";
          if (buf.length > 10e6) return undefined;
        }
        const sha = crypto.createHash("sha256").update(buf).digest("hex");
        const ext = (/\.(woff2|woff|ttf|otf|png|jpe?g|gif|webp|avif|svg)(?:[?#]|$)/i.exec(url)?.[1] ?? (type.split("/")[1] || "bin").replace(/[^a-z0-9]/gi, "").slice(0, 8)).toLowerCase();
        const file = `assets/${sha.slice(0, 16)}.${ext}`;
        await fs.writeFile(path.join(out, file), buf);
        assets[url] = { file, sha256: sha, bytes: buf.length, type };
      } catch { /* not fetched: recorded by absence */ }
      return assets[url];
    };
    const fontFaces = [];
    for (const { base: from, d } of faces) {
      let src = d.src ?? "";
      for (const m of src.matchAll(/url\(\s*(["']?)(.*?)\1\s*\)/g)) {
        const a = await fetchAsset(new URL(m[2], from).href);
        if (a) src = src.replace(m[0], `url("${a.file}")`);
      }
      fontFaces.push({ ...d, src });
    }
    const fontTokens = {};
    const metricsReq = [];
    let tn = 0;
    for (const [key, t] of Object.entries(tokens)) {
      const id = `f${++tn}`;
      fontTokens[id] = { key, family: t.family, weight: t.weight, style: t.style, platform: t.platform };
      metricsReq.push({ key: id, font: `${t.style} ${t.weight} 100px ${t.family}` });
    }
    for (const mm of await page.evaluate(metricsInPage, metricsReq)) Object.assign(fontTokens[mm.key], { ascent: mm.ascent, descent: mm.descent });

    for (const x of Object.values(extras)) if (x.src) await fetchAsset(x.src);
    const factFiles = [...wFiles.map((w) => path.join("w", `${w}.json`)), ...Object.entries(sweeps).flatMap(([w, s]) => s.ends.map((h) => path.join("h", w, `${h}.json`)))];
    for (const f of factFiles) {
      const d = JSON.parse(await fs.readFile(path.join(out, f), "utf8"));
      const styles = [...Object.values(d.s), ...Object.values(d.ps).map((p) => p.s ?? {})];
      for (const s of styles) for (const u of (s["background-image"] ?? "").matchAll(/url\("([^"]+)"\)/g)) await fetchAsset(u[1]);
    }
    const route = new URL(url);
    const meta = {
      version: 1, route: route.pathname + route.search, url: page.url(), height: o.height, range: [o.min, o.max], tolerance: o.tolerance, seed: o.seed,
      captures: cache.size, capped, widths: [...cache.keys()].sort((p, q) => p - q), segments, ends, w_files: wFiles, checks,
      heights: { range: [o.heightMin, o.heightMax], sweeps },
      props, sizing_props: SIZING_PROPS, elements: dom.elements, texts: dom.texts, extras, fonts: fontTokens, faces: fontFaces, assets,
      page_errors: errors,
      browser: { channel: CHANNEL ?? "chromium", version: browser.version() },
      ms: { search: searchMs, pass2: pass2Ms, heights: heightMs, total: Date.now() - t0 },
    };
    return meta;
  } finally {
    await browser.close();
  }
}

// A full-page PNG at each width, each from a fresh load: a full-page shot after only resizing can
// repeat the first screen.
async function screenshots(url, dir, height, widths) {
  await fs.rm(dir, { recursive: true, force: true });
  await fs.mkdir(dir, { recursive: true });
  for (const width of widths) {
    const { browser, page } = await openSettled(url, { width, height });
    try {
      await page.screenshot({ path: path.join(dir, `${width}.png`), fullPage: true, animations: "disabled" });
    } finally {
      await browser.close();
    }
  }
}

async function stamp(dir) {
  const files = {};
  const walk = async (at) => {
    for (const entry of (await fs.readdir(path.join(dir, at), { withFileTypes: true })).sort((p, q) => (p.name < q.name ? -1 : 1))) {
      const rel = at ? `${at}/${entry.name}` : entry.name;
      if (entry.isDirectory()) await walk(rel);
      else if (entry.isFile()) files[rel] = crypto.createHash("sha256").update(await fs.readFile(path.join(dir, rel))).digest("hex");
    }
  };
  await walk("");
  return { root: dir, files };
}

const { values } = parseArgs({
  options: {
    url: { type: "string" }, out: { type: "string" }, screenshots: { type: "string" }, stamp: { type: "string" },
    min: { type: "string" }, max: { type: "string" }, height: { type: "string" },
    "height-min": { type: "string" }, "height-max": { type: "string" },
  },
});
if (!values.url || !values.out) {
  console.error("usage: node capture.mjs --url URL --out DIR [--screenshots DIR] [--stamp DIR] [--min] [--max] [--height] [--height-min] [--height-max]");
  process.exit(2);
}
const number = (v, d) => {
  if (v === undefined) return d;
  const n = Number(v);
  if (!Number.isInteger(n) || n <= 0) { console.error(`capture: \`${v}\` is not a positive whole number`); process.exit(2); }
  return n;
};
const o = {
  ...DEFAULTS,
  min: number(values.min, DEFAULTS.min), max: number(values.max, DEFAULTS.max), height: number(values.height, DEFAULTS.height),
  heightMin: number(values["height-min"], DEFAULTS.heightMin), heightMax: number(values["height-max"], DEFAULTS.heightMax),
};
const source = values.stamp ? await stamp(values.stamp) : undefined;
const meta = await search(values.url, values.out, o);
if (source) meta.source = source;
await fs.writeFile(path.join(values.out, "meta.json"), JSON.stringify(meta) + "\n");
if (values.screenshots) {
  const widths = [...new Set([o.min, ...o.heightWidths.filter((w) => w >= o.min && w <= o.max), o.max])].sort((p, q) => p - q);
  await screenshots(values.url, values.screenshots, o.height, widths);
}
console.log(JSON.stringify({
  captures: meta.captures, segments: meta.segments.length, failed_checks: meta.checks.filter((c) => !c.ok).length, capped: meta.capped,
  elements: meta.elements.length, texts: meta.texts.length, assets: Object.keys(meta.assets).length, page_errors: meta.page_errors.length,
  browser: meta.browser, ms: meta.ms,
}));
