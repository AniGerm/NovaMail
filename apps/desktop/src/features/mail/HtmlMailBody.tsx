import { useMemo, useState } from "react";

import type { AttachmentDto } from "@/shared/api/types";

interface HtmlMailBodyProps {
  html: string;
  attachments?: AttachmentDto[];
  /** Optional plaintext fallback if HTML rendering is refused. */
  textFallback?: string;
}

const MAX_HTML_CHARS = 600_000;

/**
 * Prepare HTML for a plain light-DOM render.
 * Intentionally avoids iframes, Shadow DOM, and convertFileSrc — those paths
 * have crashed WebKitGTK when opening messages.
 */
export function prepareSafeMailHtml(html: string): string {
  let out = html;
  // Drop author stylesheets (layout via tables/inline styles still works).
  out = out.replace(/<style[\s\S]*?<\/style>/gi, "");
  // Broken cid: images → empty alt placeholder (no native asset protocol calls).
  out = out.replace(
    /(<img\b[^>]*\bsrc\s*=\s*)(["'])cid:[^"']*\2/gi,
    "$1$2$2",
  );
  // Cap giant leftover data: URIs.
  out = out.replace(/data:[^"'>\s]{40000,}/gi, "data:,");
  // Prevent HTML mail from painting over the whole app (fixed/sticky overlays).
  out = out.replace(/position\s*:\s*(fixed|sticky)/gi, "position:relative");
  out = out.replace(
    /(<(?:html|body)\b[^>]*\bstyle\s*=\s*["'][^"']*)\b(?:width|height|min-height)\s*:\s*[^;"']+/gi,
    "$1",
  );
  if (out.length > MAX_HTML_CHARS) {
    // Slice on a code-unit boundary that is also a tag boundary when possible.
    const cut = out.lastIndexOf(">", MAX_HTML_CHARS);
    const end = cut > MAX_HTML_CHARS / 2 ? cut + 1 : MAX_HTML_CHARS;
    out = `${out.slice(0, end)}<p style="color:#666;margin-top:1em">[…]</p>`;
  }
  return out;
}

export function HtmlMailBody({
  html,
  textFallback,
}: HtmlMailBodyProps) {
  const [failed, setFailed] = useState(false);

  const safe = useMemo(() => {
    try {
      return prepareSafeMailHtml(html);
    } catch {
      return "";
    }
  }, [html]);

  if (failed || !safe.trim()) {
    return (
      <div className="whitespace-pre-wrap text-[15px] leading-7 text-[var(--nova-ink)]">
        {textFallback?.trim() || "HTML konnte nicht dargestellt werden."}
      </div>
    );
  }

  return (
    <div
      className="nova-html-body max-w-none overflow-x-auto text-[15px] leading-7 text-[var(--nova-ink)] [&_img]:h-auto [&_img]:max-w-full [&_table]:max-w-full"
      // Sanitized in Rust; further stripped above. No iframe/Shadow DOM.
      dangerouslySetInnerHTML={{ __html: safe }}
      onErrorCapture={() => setFailed(true)}
    />
  );
}
