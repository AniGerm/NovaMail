import { useEffect, useRef, type ReactNode } from "react";
import {
  Bold,
  Indent,
  Italic,
  List,
  ListOrdered,
  Outdent,
  Underline,
} from "lucide-react";
import { Select } from "@novamail/ui";

import { useT } from "@/shared/i18n/useT";
import { useUiStore } from "@/shared/store/uiStore";

export function plainToHtml(text: string): string {
  const trimmed = text.trim();
  if (!trimmed) return "<p><br></p>";
  // Already looks like HTML markup from a previous edit session.
  if (/<[a-z][\s\S]*>/i.test(trimmed)) return text;
  return text
    .split(/\n{2,}/)
    .map((block) => `<p>${escapeHtml(block).replace(/\n/g, "<br>")}</p>`)
    .join("");
}

export function htmlToPlain(html: string): string {
  const el = document.createElement("div");
  el.innerHTML = html;
  return (el.innerText || "").replace(/\u00a0/g, " ").trimEnd();
}

function escapeHtml(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}

const FONTS = [
  { value: "IBM Plex Sans", labelKey: "fontSans" as const },
  { value: "Source Serif 4", labelKey: "fontSerif" as const },
  { value: "Georgia", labelKey: "fontGeorgia" as const },
  { value: "Arial", labelKey: "fontArial" as const },
  { value: "Times New Roman", labelKey: "fontTimes" as const },
];

const SIZES = [
  { value: "2", labelKey: "fontSizeSmall" as const },
  { value: "3", labelKey: "fontSizeNormal" as const },
  { value: "5", labelKey: "fontSizeLarge" as const },
  { value: "6", labelKey: "fontSizeXLarge" as const },
];

interface RichTextEditorProps {
  valueHtml: string;
  onChange: (html: string, plain: string) => void;
  required?: boolean;
}

function runCommand(command: string, value?: string) {
  document.execCommand("styleWithCSS", false, "true");
  document.execCommand(command, false, value);
}

export function RichTextEditor({
  valueHtml,
  onChange,
  required,
}: RichTextEditorProps) {
  const t = useT();
  const locale = useUiStore((s) => s.locale);
  const editorRef = useRef<HTMLDivElement>(null);
  const lastHtml = useRef<string>("");

  useEffect(() => {
    const el = editorRef.current;
    if (!el) return;
    if (valueHtml === lastHtml.current) return;
    el.innerHTML = valueHtml || "<p><br></p>";
    lastHtml.current = el.innerHTML;
  }, [valueHtml]);

  function emitChange() {
    const el = editorRef.current;
    if (!el) return;
    const html = el.innerHTML;
    lastHtml.current = html;
    onChange(html, htmlToPlain(html));
  }

  function withFocus(action: () => void) {
    editorRef.current?.focus();
    action();
    emitChange();
  }

  return (
    <div className="grid gap-1 text-sm">
      <span>{t("message")}</span>
      <div className="overflow-hidden rounded-[var(--nova-radius-md)] border border-[color-mix(in_srgb,var(--nova-accent)_35%,var(--nova-border))] bg-[var(--nova-surface)] focus-within:border-[var(--nova-accent)] focus-within:ring-2 focus-within:ring-[var(--nova-accent-soft)]">
        <div
          className="flex flex-wrap items-center gap-1 border-b border-[var(--nova-border)] bg-[var(--nova-surface-2)] px-2 py-1.5"
          role="toolbar"
          aria-label={t("editorToolbar")}
        >
          <ToolbarButton
            label={t("formatBold")}
            onClick={() => withFocus(() => runCommand("bold"))}
          >
            <Bold size={15} />
          </ToolbarButton>
          <ToolbarButton
            label={t("formatItalic")}
            onClick={() => withFocus(() => runCommand("italic"))}
          >
            <Italic size={15} />
          </ToolbarButton>
          <ToolbarButton
            label={t("formatUnderline")}
            onClick={() => withFocus(() => runCommand("underline"))}
          >
            <Underline size={15} />
          </ToolbarButton>

          <ToolbarDivider />

          <ToolbarButton
            label={t("formatBulletList")}
            onClick={() => withFocus(() => runCommand("insertUnorderedList"))}
          >
            <List size={15} />
          </ToolbarButton>
          <ToolbarButton
            label={t("formatNumberedList")}
            onClick={() => withFocus(() => runCommand("insertOrderedList"))}
          >
            <ListOrdered size={15} />
          </ToolbarButton>
          <ToolbarButton
            label={t("formatIndent")}
            onClick={() => withFocus(() => runCommand("indent"))}
          >
            <Indent size={15} />
          </ToolbarButton>
          <ToolbarButton
            label={t("formatOutdent")}
            onClick={() => withFocus(() => runCommand("outdent"))}
          >
            <Outdent size={15} />
          </ToolbarButton>

          <ToolbarDivider />

          <label className="flex items-center gap-1 text-xs text-[var(--nova-ink-muted)]">
            <span className="sr-only">{t("formatFont")}</span>
            <Select
              className="h-8 min-w-[8.5rem] rounded-[var(--nova-radius-sm)] px-2 py-0 text-xs"
              defaultValue="IBM Plex Sans"
              onChange={(e) =>
                withFocus(() => runCommand("fontName", e.target.value))
              }
              aria-label={t("formatFont")}
            >
              {FONTS.map((font) => (
                <option key={font.value} value={font.value}>
                  {t(font.labelKey)}
                </option>
              ))}
            </Select>
          </label>
          <label className="flex items-center gap-1 text-xs text-[var(--nova-ink-muted)]">
            <span className="sr-only">{t("formatFontSize")}</span>
            <Select
              className="h-8 min-w-[6.5rem] rounded-[var(--nova-radius-sm)] px-2 py-0 text-xs"
              defaultValue="3"
              onChange={(e) =>
                withFocus(() => runCommand("fontSize", e.target.value))
              }
              aria-label={t("formatFontSize")}
            >
              {SIZES.map((size) => (
                <option key={size.value} value={size.value}>
                  {t(size.labelKey)}
                </option>
              ))}
            </Select>
          </label>
        </div>

        <div
          ref={editorRef}
          role="textbox"
          aria-multiline="true"
          aria-required={required || undefined}
          aria-label={t("message")}
          contentEditable
          suppressContentEditableWarning
          spellCheck
          lang={locale === "de" ? "de" : "en"}
          className="nova-rich-editor min-h-[220px] max-h-[420px] overflow-y-auto px-3 py-2 text-[var(--nova-ink)] outline-none"
          onInput={emitChange}
          onBlur={emitChange}
          onPaste={(e) => {
            // Prefer plain text paste to avoid messy Word/HTML junk.
            e.preventDefault();
            const text = e.clipboardData.getData("text/plain");
            runCommand("insertText", text);
            emitChange();
          }}
        />
      </div>
      <p className="text-xs text-[var(--nova-ink-muted)]">{t("spellcheckHint")}</p>
    </div>
  );
}

function ToolbarButton({
  label,
  onClick,
  children,
}: {
  label: string;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      title={label}
      aria-label={label}
      className="inline-flex h-8 w-8 items-center justify-center rounded-[var(--nova-radius-sm)] text-[var(--nova-ink)] hover:bg-[var(--nova-accent-soft)] hover:text-[var(--nova-accent)]"
      onMouseDown={(e) => e.preventDefault()}
      onClick={onClick}
    >
      {children}
    </button>
  );
}

function ToolbarDivider() {
  return (
    <span
      aria-hidden
      className="mx-0.5 h-5 w-px bg-[var(--nova-border)]"
    />
  );
}
