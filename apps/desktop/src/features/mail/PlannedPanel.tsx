import { useEffect, useState } from "react";
import { Button } from "@novamail/ui";

import { api } from "@/shared/api/client";
import type {
  AppError,
  OutboundQueueItemDto,
  SnoozedMessageDto,
} from "@/shared/api/types";
import { useT } from "@/shared/i18n/useT";
import { formatMessageDate } from "@/shared/lib/format";
import { useUiStore } from "@/shared/store/uiStore";

export function PlannedPanel({
  onOpenMessage,
  onChanged,
}: {
  onOpenMessage: (messageId: string) => void;
  onChanged: () => void;
}) {
  const t = useT();
  const locale = useUiStore((s) => s.locale);
  const [snoozed, setSnoozed] = useState<SnoozedMessageDto[]>([]);
  const [outbound, setOutbound] = useState<OutboundQueueItemDto[]>([]);
  const [error, setError] = useState<string | null>(null);

  const refresh = async () => {
    try {
      const [s, o] = await Promise.all([
        api.messagesListSnoozed(),
        api.outboundList(),
      ]);
      setSnoozed(s);
      setOutbound(o);
      setError(null);
    } catch (err) {
      setError((err as AppError).message);
    }
  };

  useEffect(() => {
    void refresh();
  }, []);

  return (
    <section className="flex h-full min-h-0 flex-col overflow-y-auto px-6 py-5">
      <h2 className="mb-1 font-[family-name:var(--nova-font-display)] text-2xl">
        {t("planned")}
      </h2>
      <p className="mb-6 text-sm text-[var(--nova-ink-muted)]">
        {t("plannedDescription")}
      </p>
      {error ? (
        <p className="mb-4 text-sm text-[var(--nova-danger)]" role="alert">
          {error}
        </p>
      ) : null}

      <h3 className="mb-2 text-sm font-semibold uppercase tracking-wide text-[var(--nova-ink-muted)]">
        {t("snoozedSection")} ({snoozed.length})
      </h3>
      {snoozed.length === 0 ? (
        <p className="mb-6 text-sm text-[var(--nova-ink-muted)]">
          {t("snoozedEmpty")}
        </p>
      ) : (
        <ul className="mb-8 divide-y divide-[var(--nova-border)] rounded-[var(--nova-radius-md)] border border-[var(--nova-border)]">
          {snoozed.map((item) => (
            <li
              key={item.messageId}
              className="flex items-center justify-between gap-3 px-3 py-3"
            >
              <button
                type="button"
                className="min-w-0 flex-1 text-left"
                onClick={() => onOpenMessage(item.messageId)}
              >
                <p className="truncate text-sm font-medium">{item.subject}</p>
                <p className="truncate text-xs text-[var(--nova-ink-muted)]">
                  {item.fromEmail} · {t("snoozeUntil")}{" "}
                  {formatMessageDate(item.wakeAt, locale)}
                </p>
              </button>
              <Button
                type="button"
                size="sm"
                variant="secondary"
                onClick={() => {
                  void api
                    .messagesUnsnooze(item.messageId)
                    .then(() => {
                      onChanged();
                      return refresh();
                    })
                    .catch((err) => setError((err as AppError).message));
                }}
              >
                {t("unsnooze")}
              </Button>
            </li>
          ))}
        </ul>
      )}

      <h3 className="mb-2 text-sm font-semibold uppercase tracking-wide text-[var(--nova-ink-muted)]">
        {t("sendLaterSection")} ({outbound.length})
      </h3>
      {outbound.length === 0 ? (
        <p className="text-sm text-[var(--nova-ink-muted)]">
          {t("sendLaterEmpty")}
        </p>
      ) : (
        <ul className="divide-y divide-[var(--nova-border)] rounded-[var(--nova-radius-md)] border border-[var(--nova-border)]">
          {outbound.map((item) => (
            <li
              key={item.id}
              className="flex items-center justify-between gap-3 px-3 py-3"
            >
              <div className="min-w-0 flex-1">
                <p className="truncate text-sm font-medium">{item.subject}</p>
                <p className="truncate text-xs text-[var(--nova-ink-muted)]">
                  {item.toSummary || item.accountEmail} ·{" "}
                  {formatMessageDate(item.sendAt, locale)} · {item.status}
                </p>
                {item.lastError ? (
                  <p className="truncate text-xs text-[var(--nova-danger)]">
                    {item.lastError}
                  </p>
                ) : null}
              </div>
              <Button
                type="button"
                size="sm"
                variant="secondary"
                onClick={() => {
                  void api
                    .outboundCancel(item.id)
                    .then(() => {
                      onChanged();
                      return refresh();
                    })
                    .catch((err) => setError((err as AppError).message));
                }}
              >
                {t("cancelSendLater")}
              </Button>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
