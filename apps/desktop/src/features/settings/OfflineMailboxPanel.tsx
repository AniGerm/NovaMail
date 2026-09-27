import { useEffect, useMemo, useState } from "react";
import { Button, Input, Select } from "@novamail/ui";
import { HardDrive, Info } from "lucide-react";

import { api } from "@/shared/api/client";
import type {
  AccountDto,
  AccountQuotaDto,
  AppError,
  OfflineMailboxAccountPolicy,
  OfflineMailboxMode,
  OfflineMailboxSettingsDto,
} from "@/shared/api/types";
import { useT } from "@/shared/i18n/useT";

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

function defaultPolicy(accountId: string): OfflineMailboxAccountPolicy {
  return {
    accountId,
    mode: "off",
    promptThresholdPercent: 90,
    activeThresholdPercent: 85,
    keepStarredOnImap: true,
    minAgeDays: 30,
    batchLimit: 50,
    promptDismissed: false,
  };
}

export function OfflineMailboxPanel({ accounts }: { accounts: AccountDto[] }) {
  const t = useT();
  const [settings, setSettings] = useState<OfflineMailboxSettingsDto | null>(
    null,
  );
  const [selectedId, setSelectedId] = useState<string | null>(
    accounts[0]?.id ?? null,
  );
  const [quota, setQuota] = useState<AccountQuotaDto | null>(null);
  const [localCount, setLocalCount] = useState(0);
  const [status, setStatus] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (accounts.length === 0) return;
    if (!selectedId || !accounts.some((a) => a.id === selectedId)) {
      setSelectedId(accounts[0].id);
    }
  }, [accounts, selectedId]);

  useEffect(() => {
    void api
      .offlineMailboxGet()
      .then(setSettings)
      .catch((err) => setError((err as AppError).message));
  }, []);

  useEffect(() => {
    if (!selectedId) return;
    void api
      .offlineMailboxQuota(selectedId)
      .then(setQuota)
      .catch(() => setQuota(null));
    void api
      .offlineMailboxLocalCount(selectedId)
      .then(setLocalCount)
      .catch(() => setLocalCount(0));
  }, [selectedId]);

  const policy = useMemo(() => {
    if (!selectedId) return null;
    const found = settings?.accounts.find((p) => p.accountId === selectedId);
    return found ?? defaultPolicy(selectedId);
  }, [selectedId, settings]);

  const updatePolicy = (patch: Partial<OfflineMailboxAccountPolicy>) => {
    if (!policy || !selectedId) return;
    const next: OfflineMailboxAccountPolicy = { ...policy, ...patch };
    setSettings((prev) => {
      const accounts = [...(prev?.accounts ?? [])];
      const idx = accounts.findIndex((p) => p.accountId === selectedId);
      if (idx >= 0) accounts[idx] = next;
      else accounts.push(next);
      return { accounts };
    });
  };

  const save = async () => {
    if (!policy) return;
    setBusy(true);
    setError(null);
    try {
      const saved = await api.offlineMailboxSetPolicy(policy);
      setSettings((prev) => {
        const accounts = [...(prev?.accounts ?? [])];
        const idx = accounts.findIndex((p) => p.accountId === saved.accountId);
        if (idx >= 0) accounts[idx] = saved;
        else accounts.push(saved);
        return { accounts };
      });
      setStatus(t("offlineSaved"));
    } catch (err) {
      setError((err as AppError).message);
    } finally {
      setBusy(false);
    }
  };

  const runNow = async () => {
    if (!selectedId) return;
    setBusy(true);
    setError(null);
    try {
      const report = await api.offlineMailboxRun(selectedId, true);
      setStatus(
        t("offlineRunResult", {
          count: report.offloaded,
          freed: formatBytes(report.freedBytes),
        }),
      );
      const nextCount = await api.offlineMailboxLocalCount(selectedId);
      setLocalCount(nextCount);
      const nextQuota = await api.offlineMailboxQuota(selectedId);
      setQuota(nextQuota);
    } catch (err) {
      setError((err as AppError).message);
    } finally {
      setBusy(false);
    }
  };

  if (accounts.length === 0) {
    return (
      <p className="text-sm text-[var(--nova-ink-muted)]">{t("noAccountsYet")}</p>
    );
  }

  const percent = quota?.percent ?? null;
  const barWidth = Math.min(100, Math.max(0, percent ?? 0));

  return (
    <div className="grid gap-3">
      <section className="grid gap-2 rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] p-3">
        <div className="flex items-start gap-2">
          <HardDrive className="mt-0.5 h-4 w-4 shrink-0 text-[var(--nova-accent)]" />
          <div>
            <h3 className="font-medium">{t("offlineTitle")}</h3>
            <p className="text-xs text-[var(--nova-ink-muted)]">
              {t("offlineDescription")}
            </p>
          </div>
        </div>

        <label className="grid gap-1">
          <span className="text-xs">{t("accounts")}</span>
          <Select
            className="h-11"
            value={selectedId ?? ""}
            onChange={(e) => setSelectedId(e.target.value)}
          >
            {accounts.map((account) => (
              <option key={account.id} value={account.id}>
                {account.name} ({account.email})
              </option>
            ))}
          </Select>
        </label>

        {quota ? (
          <div className="grid gap-1">
            <div className="flex justify-between text-xs text-[var(--nova-ink-muted)]">
              <span>
                {t("offlineQuotaUsed", {
                  used: formatBytes(quota.usedBytes),
                  limit: quota.limitBytes
                    ? formatBytes(quota.limitBytes)
                    : t("offlineQuotaUnknown"),
                })}
              </span>
              <span>
                {percent != null
                  ? `${percent.toFixed(0)}%`
                  : t("offlineQuotaEstimate")}
                {quota.source === "server"
                  ? ` · ${t("offlineQuotaServer")}`
                  : ` · ${t("offlineQuotaEstimate")}`}
              </span>
            </div>
            <div className="h-2 overflow-hidden rounded-full bg-[var(--nova-surface-2)]">
              <div
                className="h-full rounded-full bg-[var(--nova-accent)] transition-all"
                style={{ width: `${barWidth}%` }}
              />
            </div>
            <p className="text-xs text-[var(--nova-ink-muted)]">
              {t("offlineLocalCount", { count: localCount })}
            </p>
          </div>
        ) : null}

        {policy ? (
          <>
            <label className="grid gap-1">
              <span className="text-xs">{t("offlineMode")}</span>
              <Select
                className="h-11"
                value={policy.mode}
                onChange={(e) =>
                  updatePolicy({ mode: e.target.value as OfflineMailboxMode })
                }
              >
                <option value="off">{t("offlineModeOff")}</option>
                <option value="threshold">{t("offlineModeThreshold")}</option>
                <option value="overflow">{t("offlineModeOverflow")}</option>
                <option value="alwaysPurge">{t("offlineModeAlways")}</option>
              </Select>
            </label>

            <label className="grid gap-1">
              <span className="text-xs">
                {t("offlinePromptThreshold")}: {policy.promptThresholdPercent}%
              </span>
              <input
                type="range"
                min={50}
                max={99}
                value={policy.promptThresholdPercent}
                onChange={(e) =>
                  updatePolicy({
                    promptThresholdPercent: Number(e.target.value),
                  })
                }
              />
            </label>

            <label className="grid gap-1">
              <span className="text-xs">
                {t("offlineActiveThreshold")}: {policy.activeThresholdPercent}%
              </span>
              <input
                type="range"
                min={10}
                max={99}
                value={policy.activeThresholdPercent}
                onChange={(e) =>
                  updatePolicy({
                    activeThresholdPercent: Number(e.target.value),
                  })
                }
              />
            </label>

            <label className="flex items-start gap-2 text-sm">
              <input
                type="checkbox"
                className="mt-1"
                checked={policy.keepStarredOnImap}
                onChange={(e) =>
                  updatePolicy({ keepStarredOnImap: e.target.checked })
                }
              />
              <span className="flex flex-col gap-0.5">
                <span className="inline-flex items-center gap-1">
                  {t("offlineKeepStarred")}
                  <span title={t("offlineKeepStarredHint")}>
                    <Info className="h-3.5 w-3.5 text-[var(--nova-ink-muted)]" />
                  </span>
                </span>
                <span className="text-xs text-[var(--nova-ink-muted)]">
                  {t("offlineKeepStarredHint")}
                </span>
              </span>
            </label>

            <div className="grid gap-2 sm:grid-cols-2">
              <label className="grid gap-1">
                <span className="text-xs">{t("offlineMinAge")}</span>
                <Input
                  type="number"
                  min={1}
                  max={3650}
                  value={policy.minAgeDays}
                  onChange={(e) =>
                    updatePolicy({ minAgeDays: Number(e.target.value) || 1 })
                  }
                />
              </label>
              <label className="grid gap-1">
                <span className="text-xs">{t("offlineBatchLimit")}</span>
                <Input
                  type="number"
                  min={1}
                  max={500}
                  value={policy.batchLimit}
                  onChange={(e) =>
                    updatePolicy({ batchLimit: Number(e.target.value) || 1 })
                  }
                />
              </label>
            </div>

            <div className="flex flex-wrap gap-2">
              <Button type="button" size="sm" disabled={busy} onClick={() => void save()}>
                {t("offlineSave")}
              </Button>
              <Button
                type="button"
                size="sm"
                variant="secondary"
                disabled={busy || policy.mode === "off"}
                onClick={() => void runNow()}
              >
                {t("offlineRunNow")}
              </Button>
            </div>
          </>
        ) : null}
      </section>

      {status ? (
        <p className="text-xs text-[var(--nova-accent)]" role="status">
          {status}
        </p>
      ) : null}
      {error ? (
        <p className="text-[var(--nova-danger)]" role="alert">
          {error}
        </p>
      ) : null}
    </div>
  );
}
