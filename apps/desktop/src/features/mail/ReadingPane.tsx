import { useEffect, useRef, useState } from "react";
import { open as openPath } from "@tauri-apps/plugin-shell";
import {
  Reply,
  Star,
  Forward,
  Sparkles,
  Paperclip,
  ShieldAlert,
  ShieldCheck,
  Clock3,
  Trash2,
  Archive,
  Maximize2,
  Share2,
  FolderOpen,
  Printer,
  FileDown,
  ChevronDown,
} from "lucide-react";
import { Button, EmptyState, IconButton } from "@novamail/ui";

import { SpellSuggestBar } from "@/features/composer/SpellSuggestBar";
import { HtmlMailBody } from "@/features/mail/HtmlMailBody";
import { api } from "@/shared/api/client";
import type {
  AppError,
  EventSuggestionDto,
  MessageDetailDto,
  OpenWithAppDto,
  PgpDecryptResult,
  SnoozePreset,
} from "@/shared/api/types";
import { useT } from "@/shared/i18n/useT";
import { displayName, formatRelative } from "@/shared/lib/format";
import { useUiStore } from "@/shared/store/uiStore";

type ReplyVariant = "a" | "b" | "own";

function isCommonAttachment(mime: string, filename: string): boolean {
  const m = (mime || "").toLowerCase();
  const name = filename.toLowerCase();
  if (
    m.startsWith("image/") ||
    m.startsWith("audio/") ||
    m.startsWith("video/") ||
    m.startsWith("text/")
  ) {
    return true;
  }
  if (
    m === "application/pdf" ||
    m.includes("officedocument") ||
    m.includes("msword") ||
    m.includes("spreadsheet") ||
    m.includes("presentation") ||
    m === "application/zip" ||
    m === "application/vnd.oasis.opendocument.text"
  ) {
    return true;
  }
  return /\.(pdf|png|jpe?g|gif|webp|svg|txt|md|csv|docx?|xlsx?|pptx?|odt|ods|odp|mp3|mp4|webm|zip|ics)$/i.test(
    name,
  );
}

interface ReadingPaneProps {
  message?: MessageDetailDto | null;
  aiEnabled?: boolean;
  inSpamFolder?: boolean;
  /** Hide chrome duplicated by the focus dialog header. */
  focusMode?: boolean;
  onReply: () => void;
  onForward?: () => void;
  onToggleStar: () => void;
  onDelete?: () => void;
  onArchive?: () => void;
  onMarkSpam?: () => void;
  onMarkNotSpam?: () => void;
  onSnooze?: (preset: SnoozePreset) => void;
  onCreateEvent?: (suggestion?: EventSuggestionDto) => void;
  onCreateTask?: () => void;
  onReplySent?: () => void;
  onOpenFocus?: () => void;
  /** Double-click preview ↔ fullscreen. */
  onToggleFocus?: () => void;
  /** Clear unread when the user engages with the preview (click/scroll). */
  onMarkRead?: () => void;
}

export function ReadingPane({
  message,
  aiEnabled = true,
  inSpamFolder = false,
  focusMode = false,
  onReply,
  onForward,
  onToggleStar,
  onDelete,
  onArchive,
  onMarkSpam,
  onMarkNotSpam,
  onSnooze,
  onCreateEvent,
  onCreateTask,
  onReplySent,
  onOpenFocus,
  onToggleFocus,
  onMarkRead,
}: ReadingPaneProps) {
  const t = useT();
  const locale = useUiStore((s) => s.locale);
  const spellcheckLang = useUiStore((s) => s.spellcheckLang);
  const [summary, setSummary] = useState<string | null>(null);
  const [eventSuggestions, setEventSuggestions] = useState<
    EventSuggestionDto[]
  >([]);
  const [pgpResult, setPgpResult] = useState<PgpDecryptResult | null>(null);
  const [variantA, setVariantA] = useState<string | null>(null);
  const [variantB, setVariantB] = useState<string | null>(null);
  const [activeVariant, setActiveVariant] = useState<ReplyVariant>("a");
  const [draft, setDraft] = useState("");
  const [draftCaret, setDraftCaret] = useState(0);
  const [aiBusy, setAiBusy] = useState(false);
  const [aiGenerating, setAiGenerating] = useState(false);
  const [sendBusy, setSendBusy] = useState(false);
  const [aiError, setAiError] = useState<string | null>(null);
  const [snoozeOpen, setSnoozeOpen] = useState(false);
  const [shareOpen, setShareOpen] = useState(false);
  const [attachMenuId, setAttachMenuId] = useState<string | null>(null);
  const [openWithApps, setOpenWithApps] = useState<OpenWithAppDto[]>([]);
  const [openWithLoading, setOpenWithLoading] = useState(false);
  const [invitePending, setInvitePending] = useState(false);
  const markedReadForId = useRef<string | null>(null);
  const draftRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    setSummary(null);
    setVariantA(null);
    setVariantB(null);
    setActiveVariant("a");
    setDraft("");
    setAiError(null);
    setSnoozeOpen(false);
    setShareOpen(false);
    setAttachMenuId(null);
    setOpenWithApps([]);
    setPgpResult(null);
    setEventSuggestions([]);
    setInvitePending(false);
    setAiGenerating(false);
    setDraftCaret(0);
    markedReadForId.current = null;
    const id = message?.summary.id;
    if (!id) return;
    let cancelled = false;
    let timer: number | undefined;
    // Defer secondary work so opening a message never blocks first paint.
    const start = window.setTimeout(() => {
      if (cancelled) return;
      const inviteProbe = `${(message?.bodyText ?? "").slice(0, 8000)}\n${(
        message?.attachments ?? []
      )
        .map((a) => a.filename)
        .join("\n")}`;
      const looksLikeInvite =
        (/BEGIN:VCALENDAR/i.test(inviteProbe) &&
          /METHOD:REQUEST/i.test(inviteProbe)) ||
        (message?.attachments ?? []).some(
          (a) => /\.ics$/i.test(a.filename) || /calendar/i.test(a.mime),
        );
      if (looksLikeInvite) setInvitePending(true);
      void api
        .calendarInvitationsList(true)
        .then((invites) => {
          if (!cancelled) {
            setInvitePending(
              looksLikeInvite || invites.some((i) => i.messageId === id),
            );
          }
        })
        .catch(() => undefined);
      void api
        .pgpInspectMessage(id)
        .then((result) => {
          if (!cancelled && result) setPgpResult(result);
        })
        .catch(() => undefined);
      if (!aiEnabled) return;
      let polls = 0;
      const loadInsights = () =>
        api
          .aiMessageInsights(id)
          .then((insights) => {
            if (cancelled) return true;
            if (insights.summary) setSummary(insights.summary);
            setEventSuggestions(insights.eventSuggestions ?? []);
            const a = insights.replyA ?? insights.replySuggestion ?? null;
            const b = insights.replyB ?? null;
            setVariantA(a);
            setVariantB(b);
            if (a && !draftRef.current?.matches(":focus")) {
              setActiveVariant("a");
              setDraft(a);
            }
            const done =
              !insights.incomplete && Boolean(insights.summary && a && b);
            setAiGenerating(!done);
            polls += 1;
            // Keep polling while incomplete; stop after ~2 min max.
            return done || polls >= 24;
          })
          .catch(() => true);
      void loadInsights();
      timer = window.setInterval(() => {
        void loadInsights().then((done) => {
          if (done && timer != null) window.clearInterval(timer);
        });
      }, 5000);
    }, 50);
    return () => {
      cancelled = true;
      window.clearTimeout(start);
      if (timer != null) window.clearInterval(timer);
    };
  }, [aiEnabled, message?.attachments, message?.bodyText, message?.summary.id]);

  if (!message) {
    return (
      <EmptyState
        title={t("selectMessageTitle")}
        description={t("selectMessageDescription")}
      />
    );
  }

  const current = message;
  const body =
    current.bodyText?.trim() ||
    stripHtml(current.bodyHtml ?? "") ||
    current.summary.snippet;
  const attachments = current.attachments ?? [];

  async function handleSummarize() {
    setAiBusy(true);
    setAiError(null);
    try {
      const result = await api.aiSummarizeMessage(current.summary.id, locale);
      setSummary(result.summary);
    } catch (error) {
      setAiError((error as AppError).message || t("summarizeFailed"));
    } finally {
      setAiBusy(false);
    }
  }

  async function ensureVariants(): Promise<{ a: string; b: string } | null> {
    if (variantA && variantB) {
      return { a: variantA, b: variantB };
    }
    setAiBusy(true);
    setAiError(null);
    try {
      const result = await api.aiSuggestReplies(
        current.summary.id,
        null,
        locale,
      );
      const a = result.variants[0] ?? "";
      const b = result.variants[1] ?? result.variants[0] ?? "";
      setVariantA(a || null);
      setVariantB(b || null);
      if (!a) return null;
      return { a, b: b || a };
    } catch (error) {
      setAiError((error as AppError).message || t("suggestFailed"));
      return null;
    } finally {
      setAiBusy(false);
    }
  }

  async function selectVariant(which: "a" | "b") {
    const variants = await ensureVariants();
    if (!variants) return;
    setActiveVariant(which);
    setDraft(which === "a" ? variants.a : variants.b);
    requestAnimationFrame(() => draftRef.current?.focus());
  }

  function selectOwn() {
    // Open the full composer instead of the inline mini draft.
    onReply();
  }

  async function handleSendReply() {
    const text = draft.trim();
    if (!text) {
      setAiError(t("replyDraftEmpty"));
      return;
    }
    setSendBusy(true);
    setAiError(null);
    try {
      const subject = current.summary.subject.startsWith("Re:")
        ? current.summary.subject
        : `Re: ${current.summary.subject || t("noSubject")}`;
      await api.messagesSend({
        accountId: current.summary.accountId,
        to: [{ email: current.summary.from.email, name: current.summary.from.name }],
        cc: [],
        bcc: [],
        subject,
        bodyText: text,
        bodyHtml: null,
        inReplyTo: current.messageId ?? null,
        references: [
          ...current.references,
          current.messageId ?? "",
        ].filter(Boolean),
        attachments: [],
      });
      onReplySent?.();
      if (activeVariant === "a") setVariantA(text);
      if (activeVariant === "b") setVariantB(text);
    } catch (error) {
      setAiError((error as AppError).message || t("sendFailed"));
    } finally {
      setSendBusy(false);
    }
  }

  function notePreviewEngaged() {
    const id = message?.summary.id;
    if (!id || !onMarkRead) return;
    if (!message?.summary.unread) return;
    if (markedReadForId.current === id) return;
    markedReadForId.current = id;
    onMarkRead();
  }

  async function openAttachment(attachment: {
    id: string;
    filename: string;
    mime: string;
  }) {
    try {
      await api.attachmentsOpen(
        attachment.id,
        current.summary.id,
        attachment.filename,
      );
      setAttachMenuId(null);
      setOpenWithApps([]);
    } catch (error) {
      setAiError((error as AppError).message || t("openAttachmentFailed"));
    }
  }

  async function openAttachmentMenu(attachment: {
    id: string;
    filename: string;
    mime: string;
  }) {
    const nextId = attachMenuId === attachment.id ? null : attachment.id;
    setAttachMenuId(nextId);
    setOpenWithApps([]);
    if (!nextId) return;
    setOpenWithLoading(true);
    try {
      const apps = await api.attachmentsListOpenWith(
        attachment.id,
        current.summary.id,
        attachment.filename,
      );
      setOpenWithApps(apps);
    } catch {
      setOpenWithApps([]);
    } finally {
      setOpenWithLoading(false);
    }
  }

  async function openAttachmentWithApp(
    attachment: { id: string; filename: string },
    appId: string,
  ) {
    try {
      await api.attachmentsOpenWith(
        attachment.id,
        appId,
        current.summary.id,
        attachment.filename,
      );
      setAttachMenuId(null);
      setOpenWithApps([]);
    } catch (error) {
      setAiError((error as AppError).message || t("openAttachmentFailed"));
    }
  }

  async function revealAttachment(attachment: {
    id: string;
    filename: string;
  }) {
    try {
      await api.attachmentsReveal(
        attachment.id,
        current.summary.id,
        attachment.filename,
      );
      setAttachMenuId(null);
      setOpenWithApps([]);
    } catch (error) {
      setAiError((error as AppError).message || t("revealAttachmentFailed"));
    }
  }

  async function onAttachmentChipClick(attachment: {
    id: string;
    filename: string;
    mime: string;
  }) {
    if (isCommonAttachment(attachment.mime, attachment.filename)) {
      await openAttachment(attachment);
      return;
    }
    // Uncommon / special formats → ask via the chevron menu (Open with…).
    await openAttachmentMenu(attachment);
  }

  async function exportMessage(format: "pdf" | "html") {
    if (!current) return;
    try {
      const path =
        format === "pdf"
          ? await api.messagesExportPdf(current.summary.id)
          : await api.messagesExportHtml(current.summary.id);
      await openPath(path);
      setShareOpen(false);
      setAiError(null);
    } catch (error) {
      setAiError((error as AppError).message || t("exportMessageFailed"));
    }
  }

  function printMessage() {
    if (!current) return;
    setShareOpen(false);
    const title = current.summary.subject || t("noSubject");
    const from = `${displayName(current.summary.from)} <${current.summary.from.email}>`;
    const bodyHtml = current.bodyHtml
      ? current.bodyHtml
      : `<pre style="white-space:pre-wrap;font:14px/1.5 sans-serif">${(
          current.bodyText || current.summary.snippet || ""
        )
          .replace(/&/g, "&amp;")
          .replace(/</g, "&lt;")
          .replace(/>/g, "&gt;")}</pre>`;
    const html = `<!DOCTYPE html><html><head><meta charset="utf-8"><title>${title
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/"/g, "&quot;")}</title>
      <style>body{font:15px/1.55 system-ui,sans-serif;color:#111;margin:1.5rem} h1{font-size:1.35rem} .meta{color:#555;margin-bottom:1rem}</style>
      </head><body><h1>${title
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")}</h1><p class="meta">${from
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")}</p>${bodyHtml}</body></html>`;
    const frame = document.createElement("iframe");
    frame.setAttribute("aria-hidden", "true");
    frame.style.position = "fixed";
    frame.style.right = "0";
    frame.style.bottom = "0";
    frame.style.width = "0";
    frame.style.height = "0";
    frame.style.border = "0";
    document.body.appendChild(frame);
    const doc = frame.contentDocument;
    if (!doc) {
      frame.remove();
      return;
    }
    doc.open();
    doc.write(html);
    doc.close();
    const cleanup = () => {
      frame.remove();
    };
    frame.onload = () => {
      try {
        frame.contentWindow?.focus();
        frame.contentWindow?.print();
      } finally {
        window.setTimeout(cleanup, 1000);
      }
    };
    // Some WebKit builds fire print before onload; still schedule cleanup.
    window.setTimeout(() => {
      try {
        frame.contentWindow?.focus();
        frame.contentWindow?.print();
      } catch {
        /* ignore */
      }
      window.setTimeout(cleanup, 1500);
    }, 250);
  }

  return (
    <article
      aria-label={t("readingPane")}
      className="flex h-full min-h-0 min-w-0 flex-col overflow-hidden"
      onPointerDownCapture={() => notePreviewEngaged()}
      onDoubleClick={(event) => {
        // Ignore double-clicks on interactive controls (buttons, links, inputs).
        const target = event.target as HTMLElement | null;
        if (
          target?.closest(
            "button, a, input, textarea, select, [role='menuitem'], [role='menu']",
          )
        ) {
          return;
        }
        notePreviewEngaged();
        if (onToggleFocus) {
          onToggleFocus();
        } else if (focusMode) {
          // Focus dialog closes via onOpenFocus parent wiring when absent.
        } else {
          onOpenFocus?.();
        }
      }}
    >
      <header className="shrink-0 border-b border-[var(--nova-border)] px-8 py-5">
        <div className="mb-3 flex items-start justify-between gap-4">
          {focusMode ? (
            <div className="min-w-0 flex-1" />
          ) : (
            <h2 className="max-w-3xl font-[family-name:var(--nova-font-display)] text-2xl leading-tight">
              {current.summary.subject || t("noSubject")}
            </h2>
          )}
          <div className="relative flex items-center gap-1">
            {onOpenFocus && !focusMode ? (
              <IconButton label={t("openFullscreen")} onClick={onOpenFocus}>
                <Maximize2 />
              </IconButton>
            ) : null}
            <div className="relative">
              <IconButton
                label={t("shareMessage")}
                onClick={() => {
                  setSnoozeOpen(false);
                  setShareOpen((open) => !open);
                }}
              >
                <Share2 />
              </IconButton>
              {shareOpen ? (
                <div
                  role="menu"
                  className="absolute right-0 top-full z-20 mt-1 min-w-[12.5rem] rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] bg-[var(--nova-surface)] py-1 shadow-[var(--nova-shadow)]"
                >
                  <button
                    type="button"
                    role="menuitem"
                    className="flex w-full items-center gap-2 px-3 py-2 text-left text-sm hover:bg-[var(--nova-accent-soft)]"
                    onClick={() => void exportMessage("pdf")}
                  >
                    <FileDown size={14} />
                    {t("exportPdf")}
                  </button>
                  <button
                    type="button"
                    role="menuitem"
                    className="flex w-full items-center gap-2 px-3 py-2 text-left text-sm hover:bg-[var(--nova-accent-soft)]"
                    onClick={() => void exportMessage("html")}
                  >
                    <FileDown size={14} />
                    {t("exportHtml")}
                  </button>
                  <button
                    type="button"
                    role="menuitem"
                    className="flex w-full items-center gap-2 px-3 py-2 text-left text-sm hover:bg-[var(--nova-accent-soft)]"
                    onClick={() => printMessage()}
                  >
                    <Printer size={14} />
                    {t("printMessage")}
                  </button>
                </div>
              ) : null}
            </div>
            <IconButton label={t("starMessage")} onClick={onToggleStar}>
              <Star
                className={
                  current.summary.starred
                    ? "fill-[var(--nova-warning)] text-[var(--nova-warning)]"
                    : ""
                }
              />
            </IconButton>
            <IconButton label={t("reply")} onClick={onReply}>
              <Reply />
            </IconButton>
            <IconButton label={t("forward")} onClick={onForward}>
              <Forward />
            </IconButton>
            {onArchive ? (
              <IconButton label={t("archive")} onClick={onArchive}>
                <Archive />
              </IconButton>
            ) : null}
            {onDelete ? (
              <IconButton label={t("delete")} onClick={onDelete}>
                <Trash2 />
              </IconButton>
            ) : null}
            {onSnooze ? (
              <>
                <IconButton
                  label={t("snooze")}
                  onClick={() => setSnoozeOpen((open) => !open)}
                >
                  <Clock3 />
                </IconButton>
                {snoozeOpen ? (
                  <div
                    role="menu"
                    className="absolute right-0 top-full z-20 mt-1 min-w-[11rem] rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] bg-[var(--nova-surface)] py-1 shadow-[var(--nova-shadow)]"
                  >
                    {(
                      [
                        ["laterToday", t("snoozeLaterToday")],
                        ["tomorrowMorning", t("snoozeTomorrowMorning")],
                        ["nextMonday", t("snoozeNextMonday")],
                      ] as const
                    ).map(([preset, label]) => (
                      <button
                        key={preset}
                        type="button"
                        role="menuitem"
                        className="block w-full px-3 py-2 text-left text-sm hover:bg-[var(--nova-accent-soft)]"
                        onClick={() => {
                          setSnoozeOpen(false);
                          onSnooze(preset);
                        }}
                      >
                        {label}
                      </button>
                    ))}
                  </div>
                ) : null}
              </>
            ) : null}
            {inSpamFolder ? (
              <IconButton label={t("markNotSpam")} onClick={onMarkNotSpam}>
                <ShieldCheck />
              </IconButton>
            ) : (
              <IconButton label={t("markSpam")} onClick={onMarkSpam}>
                <ShieldAlert />
              </IconButton>
            )}
            {onCreateEvent ? (
              <Button
                type="button"
                size="sm"
                variant="ghost"
                onClick={() => onCreateEvent()}
              >
                {t("asEvent")}
              </Button>
            ) : null}
            {onCreateTask ? (
              <Button type="button" size="sm" variant="ghost" onClick={onCreateTask}>
                {t("asTask")}
              </Button>
            ) : null}
          </div>
        </div>
        <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-sm text-[var(--nova-ink-muted)]">
          <span className="font-medium text-[var(--nova-ink)]">
            {displayName(current.summary.from)}
          </span>
          <span>&lt;{current.summary.from.email}&gt;</span>
          <span>·</span>
          <span>{formatRelative(current.summary.date, locale)}</span>
          {current.summary.localOnly ? (
            <span className="rounded-[var(--nova-radius-sm)] bg-[var(--nova-surface-2)] px-1.5 py-0.5 text-[10px] font-medium text-[var(--nova-ink-muted)]">
              {t("localOnlyBadge")}
            </span>
          ) : null}
          <span>·</span>
          <span>{current.summary.accountEmail}</span>
        </div>
        {aiError ? (
          <p className="mt-3 text-sm text-[var(--nova-danger)]" role="alert">
            {aiError}
          </p>
        ) : null}
        {invitePending ? (
          <p className="mt-3 text-sm text-[var(--nova-accent)]">
            {t("calendarInviteInInbox")}
          </p>
        ) : null}
      </header>

      <div
        className="min-h-0 flex-1 space-y-4 overflow-y-auto px-8 py-6"
        onScroll={() => notePreviewEngaged()}
      >
        {attachments.length > 0 ? (
          <section aria-label={t("attachments")}>
            <p className="mb-2 flex items-center gap-2 text-xs font-semibold uppercase tracking-wide text-[var(--nova-ink-muted)]">
              <Paperclip size={14} />
              {t("attachments")}
            </p>
            <ul className="flex flex-wrap gap-2">
              {attachments.map((attachment) => (
                <li key={attachment.id} className="relative">
                  <div className="flex overflow-hidden rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] bg-[var(--nova-surface)]">
                    <button
                      type="button"
                      className="px-3 py-1.5 text-left text-sm hover:bg-[var(--nova-accent-soft)]"
                      onClick={() => void onAttachmentChipClick(attachment)}
                      title={attachment.filename}
                    >
                      {attachment.filename}{" "}
                      <span className="text-[var(--nova-ink-muted)]">
                        ({formatBytes(attachment.size)})
                      </span>
                    </button>
                    <button
                      type="button"
                      className="border-l border-[var(--nova-border)] px-1.5 text-[var(--nova-ink-muted)] hover:bg-[var(--nova-accent-soft)] hover:text-[var(--nova-ink)]"
                      aria-label={t("openAttachmentMenu")}
                      aria-expanded={attachMenuId === attachment.id}
                      onClick={() => void openAttachmentMenu(attachment)}
                    >
                      <ChevronDown size={14} />
                    </button>
                  </div>
                  {attachMenuId === attachment.id ? (
                    <div
                      role="menu"
                      className="absolute left-0 top-full z-20 mt-1 flex max-h-[min(22rem,50vh)] min-w-[15rem] flex-col overflow-hidden rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] bg-[var(--nova-surface)] py-1 shadow-[var(--nova-shadow)]"
                    >
                      <button
                        type="button"
                        role="menuitem"
                        className="flex w-full shrink-0 items-center gap-2 px-3 py-2 text-left text-sm hover:bg-[var(--nova-accent-soft)]"
                        onClick={() => void openAttachment(attachment)}
                      >
                        <Paperclip size={14} />
                        {t("openAttachment")}
                      </button>
                      <div className="my-1 shrink-0 border-t border-[var(--nova-border)]" />
                      <p className="shrink-0 px-3 py-1.5 text-[10px] font-semibold uppercase tracking-wide text-[var(--nova-ink-muted)]">
                        {t("openWithSystem")}
                      </p>
                      <div className="min-h-0 flex-1 overflow-y-auto">
                        {openWithLoading ? (
                          <p className="px-3 py-2 text-sm text-[var(--nova-ink-muted)]">
                            …
                          </p>
                        ) : openWithApps.length === 0 ? (
                          <p className="px-3 pb-2 text-xs text-[var(--nova-ink-muted)]">
                            {t("noOpenWithApps")}
                          </p>
                        ) : (
                          openWithApps.map((app) => (
                            <button
                              key={app.id}
                              type="button"
                              role="menuitem"
                              className="flex w-full items-center gap-2 px-3 py-2 text-left text-sm hover:bg-[var(--nova-accent-soft)]"
                              onClick={() =>
                                void openAttachmentWithApp(attachment, app.id)
                              }
                            >
                              <Share2 size={14} />
                              <span className="min-w-0 truncate">{app.name}</span>
                            </button>
                          ))
                        )}
                      </div>
                      <div className="my-1 shrink-0 border-t border-[var(--nova-border)]" />
                      <button
                        type="button"
                        role="menuitem"
                        className="flex w-full shrink-0 items-center gap-2 px-3 py-2 text-left text-sm hover:bg-[var(--nova-accent-soft)]"
                        onClick={() => void revealAttachment(attachment)}
                      >
                        <FolderOpen size={14} />
                        {t("revealInFolder")}
                      </button>
                    </div>
                  ) : null}
                </li>
              ))}
            </ul>
          </section>
        ) : null}

        {pgpResult ? (
          <section
            aria-label={t("pgpResult")}
            className="rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] bg-[var(--nova-surface)] px-4 py-3"
          >
            <p className="mb-2 text-xs font-semibold uppercase tracking-wide text-[var(--nova-accent)]">
              {t("pgpResult")}
              {pgpResult.signatureValid === true
                ? ` · ${t("pgpSignatureValid")}`
                : pgpResult.signatureValid === false
                  ? ` · ${t("pgpSignatureInvalid")}`
                  : ""}
            </p>
            <pre className="whitespace-pre-wrap text-sm leading-6">
              {pgpResult.plaintext}
            </pre>
          </section>
        ) : null}

        <section
          aria-label={t("messageBody")}
          className="rounded-[var(--nova-radius-md)] border-2 border-[#7aa7d4] bg-[color-mix(in_srgb,var(--nova-accent-soft)_35%,var(--nova-surface))] px-5 py-5 shadow-[0_0_0_3px_color-mix(in_srgb,#7aa7d4_22%,transparent)]"
        >
          <p className="mb-3 text-xs font-semibold uppercase tracking-wide text-[var(--nova-accent)]">
            {t("messageBody")}
          </p>
          {current.bodyHtml ? (
            <HtmlMailBody
              html={current.bodyHtml}
              textFallback={body}
            />
          ) : (
            <div className="whitespace-pre-wrap text-[15px] leading-7 text-[var(--nova-ink)]">
              {body}
            </div>
          )}
        </section>

        {aiEnabled ? (
          <section
            aria-label={t("replyAssistTitle")}
            className="rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] bg-[color-mix(in_srgb,var(--nova-bg)_55%,var(--nova-surface))] px-4 py-4"
          >
            <div className="mb-2 flex items-center justify-between gap-3">
              <p className="text-sm font-medium">{t("replyAssistTitle")}</p>
              {!summary ? (
                <Button
                  type="button"
                  size="sm"
                  variant="ghost"
                  disabled={aiBusy}
                  onClick={() => void handleSummarize()}
                >
                  <Sparkles size={14} />
                  {aiBusy ? t("working") : t("summarize")}
                </Button>
              ) : null}
            </div>
            {aiGenerating ? (
              <div
                className="mb-3 flex items-center gap-2 rounded-[var(--nova-radius-sm)] border border-dashed border-[var(--nova-border)] bg-[var(--nova-surface)] px-3 py-2 text-xs text-[var(--nova-ink-muted)]"
                role="status"
              >
                <span className="inline-block h-2 w-2 animate-pulse rounded-full bg-[var(--nova-accent)]" />
                {t("aiReplyGenerating")}
              </div>
            ) : null}
            {summary ? (
              <p className="mb-3 text-sm leading-6 text-[var(--nova-ink-muted)]">
                <span className="font-medium text-[var(--nova-accent)]">{t("summary")}: </span>
                {summary}
              </p>
            ) : null}
            {eventSuggestions.length > 0 ? (
              <div className="mb-3">
                <p className="mb-1 text-xs font-semibold uppercase tracking-wide text-[var(--nova-ink-muted)]">
                  {t("eventSuggestions")}
                </p>
                <ul className="flex flex-col gap-1">
                  {eventSuggestions.map((suggestion, index) => (
                    <li key={`${suggestion.startsAt}-${index}`}>
                      <button
                        type="button"
                        className="text-left text-sm text-[var(--nova-accent)] underline underline-offset-2 hover:opacity-80"
                        onClick={() => onCreateEvent?.(suggestion)}
                      >
                        {suggestion.label}
                        {suggestion.location
                          ? ` · ${suggestion.location}`
                          : ""}
                      </button>
                    </li>
                  ))}
                </ul>
              </div>
            ) : null}
            <p className="mb-3 text-xs text-[var(--nova-ink-muted)]">
              {t("replyAssistSimpleHint")}
            </p>
            <div className="mb-3 flex flex-wrap gap-2">
              <Button
                type="button"
                size="sm"
                variant={activeVariant === "a" ? "primary" : "secondary"}
                disabled={aiBusy || sendBusy}
                onClick={() => void selectVariant("a")}
              >
                {t("replyVariantAShort")}
              </Button>
              <Button
                type="button"
                size="sm"
                variant={activeVariant === "b" ? "primary" : "secondary"}
                disabled={aiBusy || sendBusy}
                onClick={() => void selectVariant("b")}
              >
                {t("replyVariantBShort")}
              </Button>
              <Button
                type="button"
                size="sm"
                variant={activeVariant === "own" ? "primary" : "ghost"}
                disabled={aiBusy || sendBusy}
                onClick={selectOwn}
              >
                {t("replyOwn")}
              </Button>
            </div>
            <label className="grid gap-1.5">
              <span className="sr-only">{t("replyDraftPlaceholder")}</span>
              <SpellSuggestBar
                text={draft}
                caret={draftCaret}
                onApply={(from, to, replacement) => {
                  const next = draft.slice(0, from) + replacement + draft.slice(to);
                  setDraft(next);
                  const caret = from + replacement.length;
                  setDraftCaret(caret);
                  requestAnimationFrame(() => {
                    const el = draftRef.current;
                    if (!el) return;
                    el.focus();
                    el.setSelectionRange(caret, caret);
                  });
                }}
              />
              <textarea
                ref={draftRef}
                value={draft}
                spellCheck
                lang={spellcheckLang.replace("_", "-")}
                onChange={(e) => {
                  setDraft(e.target.value);
                  setDraftCaret(e.target.selectionStart ?? e.target.value.length);
                }}
                onSelect={(e) => {
                  setDraftCaret(e.currentTarget.selectionStart ?? 0);
                }}
                onKeyUp={(e) => {
                  setDraftCaret(e.currentTarget.selectionStart ?? 0);
                }}
                onClick={(e) => {
                  setDraftCaret(e.currentTarget.selectionStart ?? 0);
                }}
                placeholder={
                  aiGenerating && !draft.trim()
                    ? t("aiReplyGenerating")
                    : t("replyDraftPlaceholder")
                }
                rows={6}
                className="w-full resize-y rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] bg-[var(--nova-surface)] px-3 py-2.5 text-[15px] leading-6 text-[var(--nova-ink)] outline-none focus:border-[var(--nova-accent)] focus:ring-2 focus:ring-[color-mix(in_srgb,var(--nova-accent)_25%,transparent)]"
              />
            </label>
            <div className="mt-3 flex justify-end">
              <Button
                type="button"
                disabled={aiBusy || sendBusy || !draft.trim()}
                onClick={() => void handleSendReply()}
              >
                {sendBusy ? t("working") : t("send")}
              </Button>
            </div>
          </section>
        ) : null}
      </div>
    </article>
  );
}

function stripHtml(html: string): string {
  return html
    .replace(/<style[\s\S]*?<\/style>/gi, " ")
    .replace(/<script[\s\S]*?<\/script>/gi, " ")
    .replace(/<[^>]+>/g, " ")
    .replace(/\s+/g, " ")
    .trim();
}

function formatBytes(size: number): string {
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`;
  return `${(size / (1024 * 1024)).toFixed(1)} MB`;
}
