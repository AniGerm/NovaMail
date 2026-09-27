import { useEffect, useState } from "react";
import { Button, Input } from "@novamail/ui";

import { RecipientField } from "@/features/composer/RecipientField";
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
  initialBody?: string;
  initialSubject?: string;
  onClose: () => void;
  onSent: () => void;
  onAddToContacts?: (prefill: ContactPrefill) => void;
}

function formatAddress(addr: AddressDto): string {
  const name = addr.name?.trim();
  if (name) return `${name} <${addr.email}>`;
  return addr.email;
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
  initialBody = "",
  initialSubject,
  onClose,
  onSent,
  onAddToContacts,
}: ComposerProps) {
  const t = useT();
  const [accountId, setAccountId] = useState(accounts[0]?.id ?? "");
  const [to, setTo] = useState("");
  const [subject, setSubject] = useState("");
  const [body, setBody] = useState("");
  const [attachments, setAttachments] = useState<OutgoingAttachment[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    setAccountId((current) => current || accounts[0]?.id || "");
    setTo(replyTo ? formatAddress(replyTo.summary.from) : "");
    setSubject(
      initialSubject ??
        (replyTo ? `Re: ${replyTo.summary.subject}` : ""),
    );
    setBody(initialBody);
    setAttachments([]);
    setError(null);

    if (!replyTo && !initialBody && accounts[0]?.id) {
      api
        .signaturesList(accounts[0].id)
        .then((sigs) => {
          const def = sigs.find((s) => s.isDefault) ?? sigs[0];
          if (def && !initialBody) {
            setBody((current) => (current ? current : `\n\n${def.bodyText}`));
          }
        })
        .catch(() => undefined);
    }
  }, [open, replyTo, initialBody, initialSubject, accounts]);

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

  async function handleSend(event: React.FormEvent) {
    event.preventDefault();
    if (!accountId) {
      setError(t("addAccountBeforeSend"));
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await api.messagesSend({
        accountId,
        to: to
          .split(",")
          .map((value) => value.trim())
          .filter(Boolean)
          .map(parseRecipientToken),
        cc: [],
        bcc: [],
        subject,
        bodyText: body,
        bodyHtml: null,
        inReplyTo: replyTo?.messageId ?? null,
        references: replyTo
          ? [...replyTo.references, replyTo.messageId ?? ""].filter(Boolean)
          : [],
        attachments,
      });
      onSent();
      onClose();
      setBody("");
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
            {replyTo ? t("reply") : t("newMessage")}
          </h2>
          <Button type="button" variant="ghost" size="sm" onClick={onClose}>
            {t("close")}
          </Button>
        </header>

        <div className="grid gap-3 overflow-y-auto px-5 py-4">
          <label className="grid gap-1 text-sm">
            <span>{t("from")}</span>
            <select
              className="nova-select h-11 w-full appearance-none rounded-[var(--nova-radius-md)] border border-[color-mix(in_srgb,var(--nova-accent)_35%,var(--nova-border))] bg-[var(--nova-surface)] px-3 pr-9 text-[var(--nova-ink)] outline-none focus:border-[var(--nova-accent)] focus:ring-2 focus:ring-[var(--nova-accent-soft)]"
              value={accountId}
              onChange={(e) => setAccountId(e.target.value)}
            >
              {accounts.map((account) => (
                <option
                  key={account.id}
                  value={account.id}
                  className="bg-[var(--nova-surface)] text-[var(--nova-ink)]"
                >
                  {account.name} &lt;{account.email}&gt;
                </option>
              ))}
            </select>
          </label>
          <RecipientField
            value={to}
            onChange={setTo}
            onAddToContacts={onAddToContacts}
          />
          <label className="grid gap-1 text-sm">
            <span>{t("subject")}</span>
            <Input
              required
              value={subject}
              onChange={(e) => setSubject(e.target.value)}
            />
          </label>
          <label className="grid gap-1 text-sm">
            <span>{t("message")}</span>
            <textarea
              required
              value={body}
              onChange={(e) => setBody(e.target.value)}
              className="min-h-[220px] resize-y rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] bg-[var(--nova-surface)] px-3 py-2"
            />
          </label>
          <label className="grid gap-1 text-sm">
            <span>{t("attachments")}</span>
            <input
              type="file"
              multiple
              onChange={(e) => {
                void handleFiles(e.target.files);
              }}
            />
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
          </label>
          {error ? (
            <p className="text-sm text-[var(--nova-danger)]" role="alert">
              {error}
            </p>
          ) : null}
        </div>

        <footer className="flex justify-end gap-2 border-t border-[var(--nova-border)] px-5 py-4">
          <Button type="button" variant="secondary" onClick={onClose}>
            {t("discard")}
          </Button>
          <Button type="submit" disabled={busy}>
            {busy ? t("sending") : t("send")}
          </Button>
        </footer>
      </form>
    </div>
  );
}
