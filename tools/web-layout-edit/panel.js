// The editor's panel, injected into every load of the served page. Right-click an element to
// queue a change to its design; the panel renders the state edit.mjs pushes through
// `window.__layoutEdit.show` and sends what the operator asks for through
// `window.__layoutEditIntent`. Composing a change never blocks the edit edit.mjs is running.
// It decides nothing.

(() => {
  if (window.top !== window) return;

  // A path is spelled as capture.mjs spells it, so a selection names the node the facts hold.
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

  const round = (n) => Math.round(n * 100) / 100;

  const describe = (el) => {
    const r = el.getBoundingClientRect();
    return {
      path: cssPath(el),
      tag: el.tagName.toLowerCase(),
      text: (el.innerText ?? el.textContent ?? "").replace(/\s+/g, " ").trim().slice(0, 200),
      attributes: Object.fromEntries([...el.attributes].map((a) => [a.name, a.value])),
      box: { x: round(r.x + scrollX), y: round(r.y + scrollY), width: round(r.width), height: round(r.height) },
      viewport: { width: innerWidth, height: innerHeight },
    };
  };

  const h = (tag, props = {}, ...children) => {
    const el = Object.assign(document.createElement(tag), props);
    el.append(...children.filter((child) => child != null));
    return el;
  };

  const STYLE = `
    :host { all: initial; pointer-events: none; }
    .outline { position: fixed; z-index: 2147483646; pointer-events: none; border: 2px solid #e5484d; background: rgba(229, 72, 77, 0.08); box-sizing: border-box; }
    .panel { position: fixed; z-index: 2147483647; pointer-events: auto; top: 12px; right: 12px; width: 380px; max-height: calc(100vh - 24px); overflow: auto; box-sizing: border-box; padding: 12px; background: #fff; color: #1c1c1c; border: 1px solid #ccc; border-radius: 8px; box-shadow: 0 6px 24px rgba(0, 0, 0, 0.18); font: 13px/1.45 system-ui, sans-serif; }
    .panel.wide { width: min(96vw, 1100px); }
    h2 { margin: 0 0 8px; font-size: 14px; }
    p { margin: 0 0 8px; }
    code, pre { font: 12px ui-monospace, monospace; }
    pre { max-height: 320px; overflow: auto; padding: 8px; background: #f6f6f6; border-radius: 4px; white-space: pre; }
    textarea { box-sizing: border-box; width: 100%; min-height: 72px; margin: 4px 0 8px; font: inherit; }
    .row { display: flex; gap: 8px; justify-content: flex-end; flex-wrap: wrap; }
    button { font: inherit; padding: 4px 10px; cursor: pointer; }
    button.primary { background: #1f6feb; color: #fff; border: 1px solid #1f6feb; border-radius: 4px; }
    .muted { color: #666; }
    figure { margin: 0 0 12px; }
    figure img { width: 100%; border: 1px solid #ddd; }
    figcaption { color: #666; font-size: 12px; }
    details { margin: 0 0 8px; }
    summary { cursor: pointer; color: #666; }
    ol { margin: 6px 0 0; padding-left: 20px; }
    li { margin: 0 0 4px; }
  `;

  // `pushed` is the last state edit.mjs sent; `compose` is a local overlay, opened by a
  // right-click, for queuing one instruction. Composing never blocks what edit.mjs is doing.
  let pushed = { phase: "idle" };
  let compose = null; // { element, node } while the operator is writing an instruction
  let draft = ""; // the instruction text being composed
  let root;
  let outline;
  let panel;

  const send = (intent) => window.__layoutEditIntent(intent);

  const open = (node) => {
    compose = { element: describe(node), node };
    draft = "";
    render();
  };

  const cancel = () => { compose = null; render(); };

  // The element the outline should trace: the one being composed, else the one under review.
  const focused = () => {
    if (compose) return compose.node?.isConnected ? compose.node : resolve(compose.element.path);
    return pushed.element ? resolve(pushed.element.path) : null;
  };

  const resolve = (path) => { try { return document.querySelector(path); } catch { return null; } };

  const place = () => {
    if (!outline) return;
    const node = focused();
    outline.hidden = !node;
    if (!node) return;
    const r = node.getBoundingClientRect();
    Object.assign(outline.style, { left: `${r.left}px`, top: `${r.top}px`, width: `${r.width}px`, height: `${r.height}px` });
  };

  const summary = (element) =>
    h("p", {}, h("code", { textContent: `<${element.tag}>` }), element.text ? ` “${element.text}”` : "", h("br"),
      h("span", { className: "muted", textContent: element.path }));

  // What is queued behind whatever is on screen, so the operator can see it pile up.
  const queued = () => {
    const items = pushed.queued ?? [];
    if (!items.length) return null;
    return h("details", {},
      h("summary", { textContent: `Queued: ${items.length}` }),
      h("ol", {}, ...items.map((q) =>
        h("li", {}, h("code", { textContent: `<${q.tag}>` }), q.text ? ` “${q.text}”` : "", `: ${q.instruction}`))));
  };

  const finish = () =>
    pushed.unrecorded
      ? h("div", {},
        h("p", { className: "muted", textContent: `${pushed.unrecorded} approved edit(s) are not in the facts yet. Finish once every edit is approved.` }),
        h("div", { className: "row" }, h("button", { className: "primary", textContent: "Finish", onclick: () => send({ type: "finish" }) })))
      : null;

  const views = {
    idle: () => [
      pushed.message ? h("p", { textContent: pushed.message }) : h("p", { className: "muted", textContent: "Right-click an element to change its design." }),
      queued(),
      finish(),
    ],

    busy: () => [h("h2", { textContent: pushed.message }), h("p", { className: "muted", textContent: pushed.instruction }), queued()],

    review: () => [
      h("h2", { textContent: `Edit ${pushed.attempt}: the page now shows it` }),
      summary(pushed.element),
      h("p", {}, "“", pushed.instruction, "”"),
      h("p", { className: "muted", textContent: "Approve commits the change; reject reverts it." }),
      h("details", { open: true }, h("summary", { textContent: "Source diff" }), h("pre", { textContent: pushed.diff })),
      h("div", { className: "row" },
        h("button", { textContent: "Reject", onclick: () => send({ type: "reject" }) }),
        h("button", { className: "primary", textContent: "Approve", onclick: () => send({ type: "approve" }) })),
      queued(),
    ],

    finished: () => [
      h("h2", { textContent: "Facts recorded: before and after" }),
      h("p", { textContent: `Edits ${pushed.edits.join(", ")}` }),
      h("p", { className: "muted" }, `facts ${pushed.facts.before}`, h("br"), `→ ${pushed.facts.after}`),
      ...pushed.pairs.map(({ width, svg, file }) =>
        h("figure", {},
          h("img", { src: `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`, alt: `before and after at ${width}px` }),
          h("figcaption", { textContent: file }))),
    ],
  };

  const composeView = () => {
    const instruction = h("textarea", {
      value: draft,
      placeholder: "How should this element's design change? e.g. make this button half its current size",
      oninput: () => { draft = instruction.value; },
    });
    const parent = (compose.node?.isConnected ? compose.node : resolve(compose.element.path))?.parentElement;
    return [
      h("h2", { textContent: "Queue a change" }),
      summary(compose.element),
      instruction,
      h("div", { className: "row" },
        h("button", { textContent: "Select parent", disabled: !parent, onclick: () => open(parent) }),
        h("button", { textContent: "Cancel", onclick: cancel }),
        h("button", {
          className: "primary",
          textContent: "Add to queue",
          onclick: () => {
            if (!instruction.value.trim()) return instruction.focus();
            send({ type: "queue", element: compose.element, instruction: instruction.value.trim() });
            cancel();
          },
        })),
    ];
  };

  const render = () => {
    if (!panel) return;
    const children = (compose ? composeView() : views[pushed.phase]()).filter((child) => child != null);
    panel.className = pushed.phase === "finished" && !compose ? "panel wide" : "panel";
    panel.hidden = children.length === 0;
    panel.replaceChildren(...children);
    place();
  };

  const mount = () => {
    const host = document.createElement("web-layout-edit");
    root = host.attachShadow({ mode: "open" });
    outline = h("div", { className: "outline" });
    panel = h("div", { className: "panel" });
    root.append(h("style", { textContent: STYLE }), outline, panel);
    document.documentElement.append(host);
    new MutationObserver(() => place()).observe(document.documentElement, { childList: true, subtree: true });
    render();
  };

  window.__layoutEdit = {
    show(next) {
      pushed = next;
      render();
    },
  };

  addEventListener("contextmenu", (event) => {
    if (event.composedPath().some((node) => node instanceof Element && node.tagName === "WEB-LAYOUT-EDIT")) return;
    if (!(event.target instanceof Element)) return;
    event.preventDefault();
    open(event.target);
  }, true);
  addEventListener("keydown", (event) => { if (event.key === "Escape" && compose) cancel(); }, true);
  addEventListener("scroll", place, true);
  addEventListener("resize", place);

  if (document.readyState === "loading") addEventListener("DOMContentLoaded", mount);
  else mount();
})();
