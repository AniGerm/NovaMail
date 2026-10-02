import { useEffect, useMemo, useRef, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";

import type { AttachmentDto } from "@/shared/api/types";

interface HtmlMailBodyProps {
  html: string;
  attachments?: AttachmentDto[];
}

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
    if (!att.contentId || !att.path) continue;
    const bare = normalizeCid(att.contentId);
    if (!bare) continue;
    let src: string;
    try {
      src = convertFileSrc(att.path);
    } catch {
      continue;
    }
    for (const candidate of [`cid:${bare}`, `cid:<${bare}>`, `CID:${bare}`]) {
      if (out.includes(candidate)) {
        out = out.split(candidate).join(src);
      }
    }
  }
  return out;
}

function buildSrcDoc(bodyHtml: string): string {
  // Own CSP so remote newsletter images + inline/<style> CSS work inside the frame.
  const csp = [
    "default-src 'none'",
    "img-src data: blob: https: http: asset: http://asset.localhost https://asset.localhost",
    "style-src 'unsafe-inline'",
    "font-src data: https: http:",
    "media-src data: https: http:",
  ].join("; ");
  return `<!DOCTYPE html><html><head><meta charset="utf-8"/><meta http-equiv="Content-Security-Policy" content="${csp}"/><base target="_blank"/><style>
html,body{margin:0;padding:0;background:transparent;color:#1c2430;font:15px/1.55 ui-sans-serif,system-ui,sans-serif;word-wrap:break-word;overflow-wrap:anywhere}
img,video{max-width:100%;height:auto}
table{max-width:100%;border-collapse:collapse}
a{color:#1e5a8a}
</style></head><body>${bodyHtml}</body></html>`;
}

export function HtmlMailBody({ html, attachments }: HtmlMailBodyProps) {
  const iframeRef = useRef<HTMLIFrameElement>(null);
  const [height, setHeight] = useState(280);

  const resolved = useMemo(
    () => resolveCidImages(html, attachments),
    [html, attachments],
  );
  const srcDoc = useMemo(() => buildSrcDoc(resolved), [resolved]);

  useEffect(() => {
    setHeight(280);
  }, [srcDoc]);

  return (
    <iframe
      ref={iframeRef}
      title="message-html"
      className="w-full rounded-[var(--nova-radius-sm)] bg-white"
      sandbox="allow-same-origin allow-popups allow-popups-to-escape-sandbox"
      referrerPolicy="no-referrer"
      srcDoc={srcDoc}
      style={{ height, border: 0, display: "block" }}
      onLoad={() => {
        const doc = iframeRef.current?.contentDocument;
        const body = doc?.body;
        if (!body) return;
        const next = Math.min(Math.max(body.scrollHeight + 16, 120), 6000);
        setHeight(next);
      }}
    />
  );
}
