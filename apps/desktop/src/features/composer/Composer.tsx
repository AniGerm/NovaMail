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

interface ComposerProps {
  open: boolean;
  accounts: AccountDto[];
  replyTo?: MessageDetailDto | null;
  /** Existing draft being edited */
  draft?: MessageDetailDto | null;
  initialBody?: string;
  initialSubject?: string;
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

export function Composer({
  open,
  accounts,
  replyTo,
  draft = null,
  initialBody = "",
  initialSubject,
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
  const fileInputRef = useRef<HTMLInputElement>(null);

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
      return;
    }

    setAccountId((current) => current || accounts[0]?.id || "");
    setTo(replyTo ? formatAddress(replyTo.summary.from) : "");
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
  }, [open, replyTo, draft, initialBody, initialSubject, accounts]);

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
      await api.messagesSend({
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
      });
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

        <footer className="flex flex-wrap justify-end gap-2 border-t border-[var(--nova-border)] px-5 py-4">
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
          <Button type="submit" disabled={busy}>
            {busy ? t("sending") : t("send")}
          </Button>
        </footer>
      </form>
    </div>
  );
}
