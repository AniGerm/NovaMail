import { useCallback, useEffect, useState } from "react";
import { Button, Input } from "@novamail/ui";

import { api } from "@/shared/api/client";
import type { AppError, PgpKeyDto } from "@/shared/api/types";
import { useT } from "@/shared/i18n/useT";

export function PgpKeysPanel() {
  const t = useT();
  const [keys, setKeys] = useState<PgpKeyDto[]>([]);
  const [userId, setUserId] = useState("");
  const [importText, setImportText] = useState("");
  const [status, setStatus] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(async () => {
    try {
      setKeys(await api.pgpListKeys());
    } catch (err) {
      setError((err as AppError).message);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return (
    <section className="grid gap-3 border-t border-[var(--nova-border)] pt-4">
      <div>
        <h3 className="text-sm font-semibold">{t("pgpTitle")}</h3>
        <p className="text-xs text-[var(--nova-ink-muted)]">{t("pgpDescription")}</p>
      </div>
      {error ? (
        <p className="text-sm text-[var(--nova-danger)]" role="alert">
          {error}
        </p>
      ) : null}
      {status ? (
        <p className="text-sm text-[var(--nova-accent)]" role="status">
          {status}
        </p>
      ) : null}
      <label className="grid gap-1 text-sm">
        <span>{t("pgpUserId")}</span>
        <Input
          value={userId}
          onChange={(e) => setUserId(e.target.value)}
          placeholder='Name <you@example.com>'
        />
      </label>
      <Button
        type="button"
        disabled={busy || !userId.trim()}
        onClick={() => {
          setBusy(true);
          setError(null);
          void api
            .pgpGenerate(userId.trim())
            .then(() => {
              setStatus(t("pgpGenerated"));
              setUserId("");
              return refresh();
            })
            .catch((err) => setError((err as AppError).message))
            .finally(() => setBusy(false));
        }}
      >
        {t("pgpGenerate")}
      </Button>
      <label className="grid gap-1 text-sm">
        <span>{t("pgpImport")}</span>
        <textarea
          value={importText}
          onChange={(e) => setImportText(e.target.value)}
          rows={4}
          className="w-full rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] bg-[var(--nova-surface)] px-3 py-2 font-mono text-xs"
          placeholder="-----BEGIN PGP PUBLIC KEY BLOCK-----"
        />
      </label>
      <Button
        type="button"
        variant="secondary"
        disabled={busy || !importText.trim()}
        onClick={() => {
          setBusy(true);
          setError(null);
          void api
            .pgpImport(importText.trim())
            .then(() => {
              setStatus(t("pgpImported"));
              setImportText("");
              return refresh();
            })
            .catch((err) => setError((err as AppError).message))
            .finally(() => setBusy(false));
        }}
      >
        {t("pgpImportAction")}
      </Button>
      <ul className="divide-y divide-[var(--nova-border)] rounded-[var(--nova-radius-md)] border border-[var(--nova-border)]">
        {keys.length === 0 ? (
          <li className="px-3 py-3 text-sm text-[var(--nova-ink-muted)]">
            {t("pgpEmpty")}
          </li>
        ) : (
          keys.map((key) => (
            <li
              key={key.fingerprint}
              className="flex flex-wrap items-center justify-between gap-2 px-3 py-3"
            >
              <div className="min-w-0">
                <p className="truncate text-sm font-medium">
                  {key.userIds[0] || key.fingerprint}
                </p>
                <p className="truncate font-mono text-[11px] text-[var(--nova-ink-muted)]">
                  {key.fingerprint}
                  {key.hasSecret ? ` · ${t("pgpHasSecret")}` : ""}
                </p>
              </div>
              <div className="flex gap-2">
                <Button
                  type="button"
                  size="sm"
                  variant="secondary"
                  onClick={() => {
                    void api
                      .pgpExportPublic(key.fingerprint)
                      .then((armored) => {
                        void navigator.clipboard?.writeText(armored);
                        setStatus(t("pgpCopied"));
                      })
                      .catch((err) => setError((err as AppError).message));
                  }}
                >
                  {t("pgpCopyPublic")}
                </Button>
                <Button
                  type="button"
                  size="sm"
                  variant="ghost"
                  onClick={() => {
                    void api
                      .pgpDelete(key.fingerprint)
                      .then(refresh)
                      .catch((err) => setError((err as AppError).message));
                  }}
                >
                  {t("remove")}
                </Button>
              </div>
            </li>
          ))
        )}
      </ul>
    </section>
  );
}
