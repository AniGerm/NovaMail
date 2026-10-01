import { useEffect, useRef, useState } from "react";
import { Paperclip } from "lucide-react";
import { Button, Input, Select } from "@novamail/ui";

import { RecipientField } from "@/features/composer/RecipientField";
import {
  htmlToPlain,
  plainToHtml,
  RichTextEditor,
} from "@/features/composer/RichTextEditor";
import { api } from "@/shared/api/client";
import type {
  AccountDto,
  AddressDto,
  AppError,
  ContactPrefill,
  MessageDetailDto,
  OutgoingAttachment,
} from "@/shared/api/types";
import { useT } from "@/shared/i18n/useT";
import { useUiStore } from "@/shared/store/uiStore";

interface ComposerProps {
  open: boolean;
  accounts: AccountDto[];
  replyTo?: MessageDetailDto | null;
  /** Existing draft being edited */
  draft?: MessageDetailDto | null;
  initialBody?: string;
  initialSubject?: string;
  /** Prefill To field (e.g. from command palette) */
  initialTo?: string;
  onClose: () => void;
  onSent: () => void;
  onDraftSaved?: () => void;
  onAddToContacts?: (prefill: ContactPrefill) => void;
}

function formatAddress(addr: AddressDto): string {
  const name = addr.name?.trim();
  if (name) return `${name} <${addr.email}>`;
  return addr.email;
}

function formatAddressList(addrs: AddressDto[]): string {
  return addrs.map(formatAddress).join(", ");
}

function parseRecipientToken(token: string): AddressDto {
  const trimmed = token.trim();
  const match = trimmed.match(/^(.*?)\s*<([^>]+)>$/);
  if (match) {
    return {
      name: match[1]?.trim() || null,
      email: (match[2] ?? "").trim(),
    };
  }
  return { email: trimmed };
}

function resolveSendAt(
  preset: "laterToday" | "tomorrowMorning" | "nextMonday",
): number {
  const now = new Date();
  if (preset === "laterToday") {
    const evening = new Date(now);
    evening.setHours(18, 0, 0, 0);
    if (evening.getTime() > now.getTime()) {
      return Math.floor(evening.getTime() / 1000);
    }
    return Math.floor((now.getTime() + 3 * 60 * 60 * 1000) / 1000);
  }
  if (preset === "tomorrowMorning") {
    const tomorrow = new Date(now);
    tomorrow.setDate(tomorrow.getDate() + 1);
    tomorrow.setHours(9, 0, 0, 0);
    return Math.floor(tomorrow.getTime() / 1000);
  }
  const day = now.getDay(); // 0=Sun … 6=Sat
  const daysUntilMonday = day === 0 ? 1 : day === 1 ? 7 : 8 - day;
  const monday = new Date(now);
  monday.setDate(monday.getDate() + daysUntilMonday);
  monday.setHours(9, 0, 0, 0);
  return Math.floor(monday.getTime() / 1000);
}

export function Composer({
  open,
  accounts,
  replyTo,
  draft = null,
  initialBody = "",
  initialSubject,
  initialTo,
  onClose,
  onSent,
  onDraftSaved,
  onAddToContacts,
}: ComposerProps) {
  const t = useT();
  const [accountId, setAccountId] = useState(accounts[0]?.id ?? "");
  const [to, setTo] = useState("");
  const [subject, setSubject] = useState("");
  const [bodyHtml, setBodyHtml] = useState("<p><br></p>");
  const [bodyText, setBodyText] = useState("");
  const [attachments, setAttachments] = useState<OutgoingAttachment[]>([]);
  const [draftId, setDraftId] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [sendLaterOpen, setSendLaterOpen] = useState(false);
  const [pgpSign, setPgpSign] = useState(false);
  const [pgpEncrypt, setPgpEncrypt] = useState(false);
  const [aiBusy, setAiBusy] = useState(false);
  const [aiSuggestion, setAiSuggestion] = useState<string | null>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const locale = useUiStore((s) => s.locale);

  useEffect(() => {
    if (!open) return;
    if (draft) {
      setAccountId(draft.summary.accountId);
      setTo(formatAddressList(draft.summary.to));
      setSubject(draft.summary.subject === "(no subject)" ? "" : draft.summary.subject);
      const html = draft.bodyHtml?.trim()
        ? draft.bodyHtml
        : plainToHtml(draft.bodyText ?? "");
      setBodyHtml(html);
      setBodyText(htmlToPlain(html));
      setDraftId(draft.summary.id);
      setAttachments([]);
      setError(null);
      setStatus(null);
      setSendLaterOpen(false);
      setPgpSign(false);
      setPgpEncrypt(false);
      setAiSuggestion(null);
      return;
    }

    setAccountId((current) => current || accounts[0]?.id || "");
    setTo(
      replyTo
        ? formatAddress(replyTo.summary.from)
        : (initialTo ?? ""),
    );
    setSubject(
      initialSubject ??
        (replyTo ? `Re: ${replyTo.summary.subject}` : ""),
    );
    const html = plainToHtml(initialBody);
    setBodyHtml(html);
    setBodyText(htmlToPlain(html));
    setDraftId(null);
    setAttachments([]);
    setError(null);
    setStatus(null);
    setSendLaterOpen(false);
    setPgpSign(false);
    setPgpEncrypt(false);
    setAiSuggestion(null);

    if (!replyTo && !initialBody && accounts[0]?.id) {
      api
        .signaturesList(accounts[0].id)
        .then((sigs) => {
          const def = sigs.find((s) => s.isDefault) ?? sigs[0];
          if (def && !initialBody) {
            setBodyHtml((current) => {
              const plain = htmlToPlain(current);
              if (plain.trim()) return current;
              const next = plainToHtml(`\n\n${def.bodyText}`);
              setBodyText(htmlToPlain(next));
              return next;
            });
          }
        })
        .catch(() => undefined);
    }
  }, [open, replyTo, draft, initialBody, initialSubject, initialTo, accounts]);

  if (!open) return null;

  async function handleFiles(files: FileList | null) {
    if (!files) return;
    const next: OutgoingAttachment[] = [];
    for (const file of Array.from(files)) {
      const buffer = await file.arrayBuffer();
      const bytes = new Uint8Array(buffer);
      let binary = "";
      for (let i = 0; i < bytes.length; i += 1) {
        binary += String.fromCharCode(bytes[i]!);
      }
      next.push({
        filename: file.name,
        mime: file.type || "application/octet-stream",
        dataBase64: btoa(binary),
      });
    }
    setAttachments((prev) => [...prev, ...next]);
  }

  function recipients() {
    return to
      .split(",")
      .map((value) => value.trim())
      .filter(Boolean)
      .map(parseRecipientToken);
  }

  async function handleSaveDraft() {
    if (!accountId) {
      setError(t("addAccountBeforeSend"));
      return;
    }
    setBusy(true);
    setError(null);
    setStatus(null);
    try {
      const saved = await api.messagesSaveDraft({
        id: draftId,
        accountId,
        to: recipients(),
        cc: [],
        subject,
        bodyText,
        bodyHtml,
        inReplyTo: replyTo?.messageId ?? null,
        references: replyTo
          ? [...replyTo.references, replyTo.messageId ?? ""].filter(Boolean)
          : [],
      });
      setDraftId(saved.summary.id);
      setStatus(t("draftSaved"));
      onDraftSaved?.();
    } catch (err) {
      setError((err as AppError).message || t("draftSaveFailed"));
    } finally {
      setBusy(false);
    }
  }

  function buildSendRequest() {
    return {
      accountId,
      to: recipients(),
      cc: [],
      bcc: [],
      subject,
      bodyText,
      bodyHtml,
      inReplyTo: replyTo?.messageId ?? null,
      references: replyTo
        ? [...replyTo.references, replyTo.messageId ?? ""].filter(Boolean)
        : [],
      attachments,
      draftId,
      pgpSign,
      pgpEncrypt,
    };
  }

  async function handleSend(event: React.FormEvent) {
    event.preventDefault();
    if (!accountId) {
      setError(t("addAccountBeforeSend"));
      return;
    }
    if (!bodyText.trim()) {
      setError(t("messageEmpty"));
      return;
    }
    setBusy(true);
    setError(null);
    setStatus(null);
    try {
      await api.messagesSend(buildSendRequest());
      onSent();
      onClose();
      setBodyHtml("<p><br></p>");
      setBodyText("");
      setDraftId(null);
      setAttachments([]);
    } catch (err) {
      setError((err as AppError).message || t("sendFailed"));
    } finally {
      setBusy(false);
    }
  }

  async function handleSendLater(sendAt: number) {
    if (!accountId) {
      setError(t("addAccountBeforeSend"));
      return;
    }
    if (!bodyText.trim()) {
      setError(t("messageEmpty"));
      return;
    }
    setBusy(true);
    setError(null);
    setStatus(null);
    setSendLaterOpen(false);
    try {
      await api.messagesSendLater({
        sendAt,
        message: buildSendRequest(),
      });
      setStatus(t("sendLaterDone"));
      onSent();
      onClose();
      setBodyHtml("<p><br></p>");
      setBodyText("");
      setDraftId(null);
      setAttachments([]);
    } catch (err) {
      setError((err as AppError).message || t("sendFailed"));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div
      className="fixed inset-0 z-50 flex items-end justify-center bg-[rgba(14,17,20,0.35)] p-4 backdrop-blur-sm sm:items-center"
      role="dialog"
      aria-modal="true"
      aria-labelledby="composer-title"
    >
      <form
        onSubmit={handleSend}
        className="nova-fade-in flex max-h-[90vh] w-full max-w-3xl flex-col rounded-[var(--nova-radius-lg)] border border-[var(--nova-border)] bg-[var(--nova-surface)] shadow-[var(--nova-shadow)]"
      >
        <header className="flex items-center justify-between border-b border-[var(--nova-border)] px-5 py-4">
          <h2
            id="composer-title"
            className="font-[family-name:var(--nova-font-display)] text-xl"
          >
            {draftId ? t("editDraft") : replyTo ? t("reply") : t("newMessage")}
          </h2>
          <Button type="button" variant="ghost" size="sm" onClick={onClose}>
            {t("close")}
          </Button>
        </header>

        <div className="grid gap-3 overflow-y-auto px-5 py-4">
          <label className="grid gap-1 text-sm">
            <span>{t("from")}</span>
            <Select
              className="h-11"
              value={accountId}
              onChange={(e) => setAccountId(e.target.value)}
            >
              {accounts.map((account) => (
                <option key={account.id} value={account.id}>
                  {account.name} &lt;{account.email}&gt;
                </option>
              ))}
            </Select>
          </label>
          <RecipientField
            value={to}
            onChange={setTo}
            onAddToContacts={onAddToContacts}
          />
          <label className="grid gap-1 text-sm">
            <span>{t("subject")}</span>
            <Input
              value={subject}
              onChange={(e) => setSubject(e.target.value)}
              placeholder={t("subject")}
            />
          </label>
          <RichTextEditor
            required
            valueHtml={bodyHtml}
            onChange={(html, plain) => {
              setBodyHtml(html);
              setBodyText(plain);
            }}
          />
          <div className="flex flex-wrap items-center gap-2">
            <Button
              type="button"
              size="sm"
              variant="secondary"
              disabled={aiBusy || busy || !bodyText.trim()}
              onClick={() => {
                setAiBusy(true);
                setError(null);
                void api
                  .aiOptimizeDraft({
                    subject,
                    bodyText,
                    preferredLanguage: locale,
                  })
                  .then((result) => {
                    setAiSuggestion(result.suggestion.trim());
                  })
                  .catch((err) => {
                    setError((err as AppError).message || t("composeAiFailed"));
                  })
                  .finally(() => setAiBusy(false));
              }}
            >
              {aiBusy ? t("working") : t("composeAiImprove")}
            </Button>
            <span className="text-xs text-[var(--nova-ink-muted)]">
              {t("composeAiImproveHint")}
            </span>
          </div>
          {aiSuggestion ? (
            <div className="rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] bg-[color-mix(in_srgb,var(--nova-accent-soft)_40%,var(--nova-surface))] p-3">
              <p className="mb-2 text-xs font-semibold uppercase tracking-wide text-[var(--nova-accent)]">
                {t("composeAiSuggestion")}
              </p>
              <pre className="mb-3 max-h-48 overflow-y-auto whitespace-pre-wrap text-sm leading-6 text-[var(--nova-ink)]">
                {aiSuggestion}
              </pre>
              <div className="flex flex-wrap gap-2">
                <Button
                  type="button"
                  size="sm"
                  onClick={() => {
                    const html = plainToHtml(aiSuggestion);
                    setBodyHtml(html);
                    setBodyText(htmlToPlain(html));
                    setAiSuggestion(null);
                    setStatus(t("composeAiAccepted"));
                  }}
                >
                  {t("composeAiAccept")}
                </Button>
                <Button
                  type="button"
                  size="sm"
                  variant="secondary"
                  onClick={() => setAiSuggestion(null)}
                >
                  {t("composeAiReject")}
                </Button>
              </div>
            </div>
          ) : null}
          <div className="grid gap-1 text-sm">
            <span>{t("attachments")}</span>
            <input
              ref={fileInputRef}
              type="file"
              multiple
              className="sr-only"
              onChange={(e) => {
                void handleFiles(e.target.files);
                e.target.value = "";
              }}
            />
            <Button
              type="button"
              variant="secondary"
              size="sm"
              className="nova-file-btn w-fit"
              onClick={() => fileInputRef.current?.click()}
            >
              <Paperclip size={16} />
              {t("chooseAttachments")}
            </Button>
            {attachments.length > 0 ? (
              <ul className="mt-1 space-y-1 text-[var(--nova-ink-muted)]">
                {attachments.map((file) => (
                  <li key={`${file.filename}-${file.dataBase64.length}`}>
                    {file.filename}
                    <button
                      type="button"
                      className="ml-2 text-[var(--nova-accent)]"
                      onClick={() =>
                        setAttachments((prev) =>
                          prev.filter((item) => item !== file),
                        )
                      }
                    >
                      {t("remove")}
                    </button>
                  </li>
                ))}
              </ul>
            ) : null}
          </div>
          {status ? (
            <p className="text-sm text-[var(--nova-accent)]" role="status">
              {status}
            </p>
          ) : null}
          {error ? (
            <p className="text-sm text-[var(--nova-danger)]" role="alert">
              {error}
            </p>
          ) : null}
        </div>

        <footer className="flex flex-wrap items-center justify-end gap-2 border-t border-[var(--nova-border)] px-5 py-4">
          <label className="mr-auto flex items-center gap-2 text-sm">
            <input
              type="checkbox"
              checked={pgpSign}
              onChange={(e) => setPgpSign(e.target.checked)}
            />
            {t("pgpSign")}
          </label>
          <label className="flex items-center gap-2 text-sm">
            <input
              type="checkbox"
              checked={pgpEncrypt}
              onChange={(e) => setPgpEncrypt(e.target.checked)}
            />
            {t("pgpEncrypt")}
          </label>
          <Button type="button" variant="secondary" onClick={onClose}>
            {t("discard")}
          </Button>
          <Button
            type="button"
            variant="secondary"
            className="nova-file-btn"
            disabled={busy}
            onClick={() => {
              void handleSaveDraft();
            }}
          >
            {busy ? t("savingDraft") : t("saveDraft")}
          </Button>
          <div className="relative">
            <Button
              type="button"
              variant="secondary"
              disabled={busy}
              onClick={() => setSendLaterOpen((open) => !open)}
            >
              {t("sendLater")}
            </Button>
            {sendLaterOpen ? (
              <div
                role="menu"
                className="absolute bottom-full right-0 z-20 mb-1 min-w-[12rem] rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] bg-[var(--nova-surface)] py-1 shadow-[var(--nova-shadow)]"
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
                      void handleSendLater(resolveSendAt(preset));
                    }}
                  >
                    {label}
                  </button>
                ))}
              </div>
            ) : null}
          </div>
          <Button type="submit" disabled={busy}>
            {busy ? t("sending") : t("send")}
          </Button>
        </footer>
      </form>
    </div>
  );
}
