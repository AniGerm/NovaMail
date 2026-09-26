import { useEffect, useState } from "react";
import { Button, Input } from "@novamail/ui";

import { api } from "@/shared/api/client";
import type { AccountDto, AppError, MessageDetailDto } from "@/shared/api/types";

interface ComposerProps {
  open: boolean;
  accounts: AccountDto[];
  replyTo?: MessageDetailDto | null;
  initialBody?: string;
  initialSubject?: string;
  onClose: () => void;
  onSent: () => void;
}

export function Composer({
  open,
  accounts,
  replyTo,
  initialBody = "",
  initialSubject,
  onClose,
  onSent,
}: ComposerProps) {
  const [accountId, setAccountId] = useState(accounts[0]?.id ?? "");
  const [to, setTo] = useState("");
  const [subject, setSubject] = useState("");
  const [body, setBody] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    setAccountId((current) => current || accounts[0]?.id || "");
    setTo(replyTo?.summary.from.email ?? "");
    setSubject(
      initialSubject ??
        (replyTo ? `Re: ${replyTo.summary.subject}` : ""),
    );
    setBody(initialBody);
    setError(null);
  }, [open, replyTo, initialBody, initialSubject, accounts]);

  if (!open) return null;

  async function handleSend(event: React.FormEvent) {
    event.preventDefault();
    if (!accountId) {
      setError("Add an account before sending.");
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
          .map((email) => ({ email })),
        cc: [],
        bcc: [],
        subject,
        bodyText: body,
        bodyHtml: null,
        inReplyTo: replyTo?.messageId ?? null,
        references: replyTo
          ? [...replyTo.references, replyTo.messageId ?? ""].filter(Boolean)
          : [],
      });
      onSent();
      onClose();
      setBody("");
    } catch (err) {
      setError((err as AppError).message || "Send failed");
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
            {replyTo ? "Reply" : "New message"}
          </h2>
          <Button type="button" variant="ghost" size="sm" onClick={onClose}>
            Close
          </Button>
        </header>

        <div className="grid gap-3 overflow-y-auto px-5 py-4">
          <label className="grid gap-1 text-sm">
            <span>From</span>
            <select
              className="h-11 rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] bg-[var(--nova-surface)] px-3"
              value={accountId}
              onChange={(e) => setAccountId(e.target.value)}
            >
              {accounts.map((account) => (
                <option key={account.id} value={account.id}>
                  {account.name} &lt;{account.email}&gt;
                </option>
              ))}
            </select>
          </label>
          <label className="grid gap-1 text-sm">
            <span>To</span>
            <Input required value={to} onChange={(e) => setTo(e.target.value)} />
          </label>
          <label className="grid gap-1 text-sm">
            <span>Subject</span>
            <Input
              required
              value={subject}
              onChange={(e) => setSubject(e.target.value)}
            />
          </label>
          <label className="grid gap-1 text-sm">
            <span>Message</span>
            <textarea
              required
              value={body}
              onChange={(e) => setBody(e.target.value)}
              className="min-h-[220px] resize-y rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] bg-[var(--nova-surface)] px-3 py-2"
            />
          </label>
          {error ? (
            <p className="text-sm text-[var(--nova-danger)]" role="alert">
              {error}
            </p>
          ) : null}
        </div>

        <footer className="flex justify-end gap-2 border-t border-[var(--nova-border)] px-5 py-4">
          <Button type="button" variant="secondary" onClick={onClose}>
            Discard
          </Button>
          <Button type="submit" disabled={busy}>
            {busy ? "Sending…" : "Send"}
          </Button>
        </footer>
      </form>
    </div>
  );
}
