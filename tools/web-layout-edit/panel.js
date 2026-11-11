// The editor's panel, injected into every load of the served page. Right-click selects an
// element; the panel renders the state edit.mjs pushes through `window.__layoutEdit.show` and
// sends what the operator asks for through `window.__layoutEditIntent`. It decides nothing.

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
    :host { all: initial; }
    .outline { position: fixed; z-index: 2147483646; pointer-events: none; border: 2px solid #e5484d; background: rgba(229, 72, 77, 0.08); box-sizing: border-box; }
    .panel { position: fixed; z-index: 2147483647; top: 12px; right: 12px; width: 380px; max-height: calc(100vh - 24px); overflow: auto; box-sizing: border-box; padding: 12px; background: #fff; color: #1c1c1c; border: 1px solid #ccc; border-radius: 8px; box-shadow: 0 6px 24px rgba(0, 0, 0, 0.18); font: 13px/1.45 system-ui, sans-serif; }
    .panel.wide { width: min(96vw, 1100px); }
    h2 { margin: 0 0 8px; font-size: 14px; }
    p { margin: 0 0 8px; }
    code, pre { font: 12px ui-monospace, monospace; }
    pre { max-height: 320px; overflow: auto; padding: 8px; background: #f6f6f6; border-radius: 4px; white-space: pre; }
    textarea { box-sizing: border-box; width: 100%; min-height: 72px; margin: 4px 0 8px; font: inherit; }
    .row { display: flex; gap: 8px; justify-content: flex-end; flex-wrap: wrap; }
    button { font: inherit; padding: 4px 10px; cursor: pointer; }
    button.primary { background: #1f6feb; color: #fff; border: 1px solid #1f6feb; border-radius: 4px; }
    .error { color: #b42318; white-space: pre-wrap; }
    .muted { color: #666; }
    figure { margin: 0 0 12px; }
    figure img { width: 100%; border: 1px solid #ddd; }
    figcaption { color: #666; font-size: 12px; }
  `;

  let state = { phase: "idle" };
  let selected = null;
  let root;
  let outline;
  let panel;

  const send = (intent) => window.__layoutEditIntent(intent);

  const select = (el) => {
    selected = el;
    state = { phase: "select", element: describe(el), instruction: state.instruction ?? "" };
    render();
  };

  const close = () => {
    selected = null;
    state = { phase: "idle" };
    render();
    send({ type: "close" });
  };

  const place = () => {
    if (!outline) return;
    const shown = state.phase !== "idle" && selected?.isConnected;
    outline.hidden = !shown;
    if (!shown) return;
    const r = selected.getBoundingClientRect();
    Object.assign(outline.style, { left: `${r.left}px`, top: `${r.top}px`, width: `${r.width}px`, height: `${r.height}px` });
  };

  const summary = (element) =>
    h("p", {}, h("code", { textContent: `<${element.tag}>` }), element.text ? ` “${element.text}”` : "", h("br"),
      h("span", { className: "muted", textContent: element.path }));

  const views = {
    idle: () => (state.message ? [h("p", { textContent: state.message })] : []),

    busy: () => [h("h2", { textContent: state.message }), h("p", { className: "muted", textContent: state.instruction })],

    select: () => {
      const instruction = h("textarea", {
        value: state.instruction ?? "",
        placeholder: "How should this element's design change? e.g. make this button half its current size",
        oninput: () => { state.instruction = instruction.value; },
      });
      const parent = selected?.parentElement;
      return [
        h("h2", { textContent: "Selected element" }),
        summary(state.element),
        state.error ? h("p", { className: "error", textContent: state.error }) : null,
        instruction,
        h("div", { className: "row" },
          h("button", { textContent: "Select parent", disabled: !parent, onclick: () => select(parent) }),
          h("button", { textContent: "Close", onclick: close }),
          h("button", {
            className: "primary",
            textContent: "Apply",
            onclick: () => {
              if (!instruction.value.trim()) return instruction.focus();
              send({ type: "apply", element: state.element, instruction: instruction.value.trim() });
            },
          })),
      ];
    },

    review: () => [
      h("h2", { textContent: `Edit ${state.attempt}: the page now shows it` }),
      summary(state.element),
      h("p", {}, "“", state.instruction, "”"),
      state.error ? h("p", { className: "error", textContent: state.error }) : null,
      h("p", { className: "muted", textContent: "Approve captures the page's facts and commits the change; reject reverts it." }),
      h("details", { open: true }, h("summary", { textContent: "Source diff" }), h("pre", { textContent: state.diff })),
      h("div", { className: "row" },
        h("button", { textContent: "Reject", onclick: () => send({ type: "reject" }) }),
        h("button", { className: "primary", textContent: "Approve", onclick: () => send({ type: "approve" }) })),
    ],

    approved: () => [
      h("h2", { textContent: `Edit ${state.attempt} approved: before and after` }),
      summary(state.element),
      h("p", {}, "“", state.instruction, "”"),
      h("p", { className: "muted" }, `facts ${state.facts.before}`, h("br"), `→ ${state.facts.after}`),
      ...state.pairs.map(({ width, svg, file }) =>
        h("figure", {},
          h("img", { src: `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`, alt: `before and after at ${width}px` }),
          h("figcaption", { textContent: file }))),
      h("div", { className: "row" }, h("button", { className: "primary", textContent: "Close", onclick: close })),
    ],
  };

  // The element a pushed state names, once the page has rendered it.
  const find = () => {
    if (!state.element || selected?.isConnected) return false;
    try { selected = document.querySelector(state.element.path); } catch { selected = null; }
    return selected !== null;
  };

  const render = () => {
    if (!panel) return;
    find();
    const children = views[state.phase]().filter((child) => child != null);
    panel.className = state.phase === "approved" ? "panel wide" : "panel";
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
    new MutationObserver(() => { if (find()) render(); }).observe(document.documentElement, { childList: true, subtree: true });
    render();
  };

  window.__layoutEdit = {
    show(next) {
      state = next;
      if (state.phase === "idle") selected = null;
      render();
    },
  };

  addEventListener("contextmenu", (event) => {
    if (event.composedPath().some((node) => node instanceof Element && node.tagName === "WEB-LAYOUT-EDIT")) return;
    if (state.phase === "busy" || state.phase === "review") return;
    if (!(event.target instanceof Element)) return;
    event.preventDefault();
    select(event.target);
  }, true);
  addEventListener("keydown", (event) => { if (event.key === "Escape" && state.phase === "select") close(); }, true);
  addEventListener("scroll", place, true);
  addEventListener("resize", place);

  if (document.readyState === "loading") addEventListener("DOMContentLoaded", mount);
  else mount();
})();
