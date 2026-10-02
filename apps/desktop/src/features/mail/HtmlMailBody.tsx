import { useEffect, useMemo, useRef } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";

import type { AttachmentDto } from "@/shared/api/types";

interface HtmlMailBodyProps {
  html: string;
  attachments?: AttachmentDto[];
}

/** Soft cap — multi‑MB newsletters previously killed the WebKit process. */
const MAX_HTML_CHARS = 1_500_000;

function normalizeCid(cid: string): string {
  return cid.trim().replace(/^<|>$/g, "");
}

/** Resolve cid:… references to local asset URLs (avoids huge base64 in the DOM). */
export function resolveCidImages(
  html: string,
  attachments: AttachmentDto[] | undefined,
): string {
  if (!html || !attachments?.length || !html.toLowerCase().includes("cid:")) {
    return html;
  }
  let out = html;
  for (const att of attachments) {
    const path = att.path?.trim();
    if (!att.contentId || !path) continue;
    const bare = normalizeCid(att.contentId);
    if (!bare) continue;
    let src: string;
    try {
      src = convertFileSrc(path);
    } catch {
      continue;
    }
    if (!src) continue;
    for (const candidate of [`cid:${bare}`, `cid:<${bare}>`, `CID:${bare}`]) {
      if (out.includes(candidate)) {
        out = out.split(candidate).join(src);
      }
    }
  }
  return out;
}

function capHtml(html: string): string {
  if (html.length <= MAX_HTML_CHARS) return html;
  // Prefer cutting after a tag boundary so markup stays roughly well-formed.
  const cut = html.lastIndexOf(">", MAX_HTML_CHARS);
  const end = cut > MAX_HTML_CHARS / 2 ? cut + 1 : MAX_HTML_CHARS;
  return `${html.slice(0, end)}<p style="margin-top:1em;color:#666;font:14px sans-serif">[…]</p>`;
}

/**
 * Render sanitized HTML mail in a Shadow DOM.
 *
 * Avoids sandboxed `srcDoc` iframes (known WebKitGTK crash on open) while still
 * isolating newsletter `<style>` from the app chrome.
 */
export function HtmlMailBody({ html, attachments }: HtmlMailBodyProps) {
  const hostRef = useRef<HTMLDivElement>(null);

  const resolved = useMemo(() => {
    try {
      return capHtml(resolveCidImages(html, attachments));
    } catch {
      return capHtml(html);
    }
  }, [html, attachments]);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;

    let root = host.shadowRoot;
    if (!root) {
      try {
        root = host.attachShadow({ mode: "open" });
      } catch {
        // Extremely old engines — fall back to light DOM.
        host.innerHTML = resolved;
        return;
      }
    }

    try {
      root.innerHTML = `
<style>
  :host { display: block; }
  .mail {
    color: #1c2430;
    font: 15px/1.55 ui-sans-serif, system-ui, sans-serif;
    word-wrap: break-word;
    overflow-wrap: anywhere;
  }
  .mail img, .mail video { max-width: 100%; height: auto; }
  .mail table { max-width: 100%; border-collapse: collapse; }
  .mail a { color: #1e5a8a; }
</style>
<div class="mail">${resolved}</div>`;
    } catch {
      host.textContent = "HTML konnte nicht dargestellt werden.";
    }
  }, [resolved]);

  return (
    <div
      ref={hostRef}
      className="nova-html-body min-h-[8rem] w-full overflow-x-auto rounded-[var(--nova-radius-sm)] bg-white px-1 py-1"
    />
  );
}
