// The element inventory, installed into every document of a run.
//
// `install` is passed to `context.addInitScript`, so it must stay
// self-contained. It defines `window.__c2b` with `inventory()` and
// `describe(el)`. The element path is the algorithm of ui-ir
// tools/capture/src/collect.ts#L191-L199, so an element seen here and the
// composition node built from computed.json share a path, and so a node id.

export function install() {
  // An element keeps the path it had when first seen in this document: a
  // portal added later (a sheet, a menu) shifts its siblings' nth-of-type,
  // but the element is the same node of the capture.
  const firstSeen = new WeakMap();
  const rawPath = (el) => {
    const parts = [];
    for (let e = el; e && e.nodeType === 1; e = e.parentElement) {
      const tag = e.tagName.toLowerCase();
      if (!e.parentElement) {
        parts.unshift(tag);
        break;
      }
      const sib = [...e.parentElement.children].filter((s) => s.tagName === e.tagName);
      parts.unshift(sib.length > 1 ? `${tag}:nth-of-type(${sib.indexOf(e) + 1})` : tag);
    }
    return parts.join(" > ");
  };
  const cssPath = (el) => {
    if (!firstSeen.has(el)) firstSeen.set(el, rawPath(el));
    return firstSeen.get(el);
  };
  const TEXT_ROLES = new Set(["button", "link", "menuitem", "menuitemcheckbox", "menuitemradio", "option", "tab"]);
  const INPUT_ROLE = { checkbox: "checkbox", radio: "radio", submit: "button", button: "button", reset: "button", image: "button" };
  const roleOf = (el) => {
    const explicit = el.getAttribute("role");
    if (explicit) return explicit.split(/\s+/)[0];
    const tag = el.tagName.toLowerCase();
    if (tag === "a") return el.hasAttribute("href") ? "link" : null;
    if (tag === "button") return "button";
    if (tag === "form") return "form";
    if (tag === "select") return "combobox";
    if (tag === "textarea") return "textbox";
    if (tag === "input") {
      const type = (el.getAttribute("type") || "text").toLowerCase();
      if (type === "hidden") return null;
      return INPUT_ROLE[type] || "textbox";
    }
    return null;
  };
  const OVERLAY = "[role=dialog],[role=alertdialog],[role=menu],[role=listbox],[data-radix-popper-content-wrapper]";
  const visible = (el) => {
    const r = el.getBoundingClientRect();
    const s = getComputedStyle(el);
    return r.width > 0 && r.height > 0 && s.visibility !== "hidden" && s.display !== "none";
  };
  const describe = (el) => {
    const role = roleOf(el);
    const tag = el.tagName.toLowerCase();
    const type = el.getAttribute("type");
    const form = el.closest("form");
    return {
      path: cssPath(el),
      tag,
      role,
      text: role && TEXT_ROLES.has(role) ? (el.textContent || "").replace(/\s+/g, " ").trim() : null,
      href: el.getAttribute("href"),
      name: el.getAttribute("name"),
      type,
      in_overlay: !!el.parentElement?.closest(OVERLAY),
      form: form && form !== el ? cssPath(form) : null,
      submitter: (tag === "button" && (type === null || type === "submit") && !!form) || (tag === "input" && type === "submit"),
      visible: visible(el),
    };
  };
  const SELECTOR = "a[href],button,input,select,textarea,form,[role=button],[role=link],[role=menuitem],[role=menuitemcheckbox],[role=menuitemradio],[role=tab],[role=option]";
  const inventory = () => ({
    elements: [...document.querySelectorAll(SELECTOR)].map(describe).filter((e) => e.role),
    overlays: [...document.querySelectorAll("[role=dialog],[role=alertdialog],[role=menu]")]
      .filter(visible)
      .map((el) => ({ role: el.getAttribute("role"), path: cssPath(el) })),
    text: (document.body?.innerText || "")
      .split("\n")
      .map((l) => l.replace(/\s+/g, " ").trim())
      .filter(Boolean),
  });
  Object.defineProperty(window, "__c2b", { value: { inventory, describe }, enumerable: false });
}
