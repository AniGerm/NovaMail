/** Keep the webview document from scrolling and recover a collapsed shell. */

let installed = false;

function scrollableOverflow(el: Element): boolean {
  const style = window.getComputedStyle(el);
  const y = style.overflowY;
  if (y !== "auto" && y !== "scroll" && y !== "overlay") return false;
  return el.scrollHeight > el.clientHeight + 1;
}

function scrollableAncestor(target: EventTarget | null): HTMLElement | null {
  let el = target instanceof Element ? target : null;
  while (el && el !== document.body && el !== document.documentElement) {
    if (scrollableOverflow(el)) return el as HTMLElement;
    el = el.parentElement;
  }
  return null;
}

function pinDocument() {
  if (window.scrollX !== 0 || window.scrollY !== 0) {
    window.scrollTo(0, 0);
  }
  document.documentElement.scrollTop = 0;
  document.documentElement.scrollLeft = 0;
  document.body.scrollTop = 0;
  document.body.scrollLeft = 0;
}

/** Shell shorter than the webview, or the document scrolled: force a relayout. */
export function repairViewport() {
  pinDocument();
  const shell = document.querySelector("[data-app-shell]");
  const winH = window.innerHeight;
  const shellH =
    shell instanceof HTMLElement ? shell.getBoundingClientRect().height : winH;
  const collapsed = shell instanceof HTMLElement && shellH < winH - 8;
  if (!collapsed && window.scrollY === 0) return;
  const root = document.getElementById("root");
  if (!root) return;
  // Opacity (not transform) drops a stuck WebKit layer without creating a new one.
  root.style.opacity = "0.999";
  void root.offsetHeight;
  window.requestAnimationFrame(() => {
    root.style.opacity = "";
    pinDocument();
  });
}

export function installViewportLock() {
  if (installed || typeof window === "undefined") return;
  installed = true;
  if ("scrollRestoration" in history) {
    history.scrollRestoration = "manual";
  }
  pinDocument();
  window.addEventListener("resize", pinDocument);
  window.visualViewport?.addEventListener("resize", pinDocument);
  window.visualViewport?.addEventListener("scroll", pinDocument);
  document.addEventListener("scroll", pinDocument, true);
  window.addEventListener(
    "wheel",
    (event) => {
      if (scrollableAncestor(event.target)) return;
      event.preventDefault();
    },
    { capture: true, passive: false },
  );
  window.addEventListener("focus", () => repairViewport());
}
