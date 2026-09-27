import { useEffect, useRef, useState } from "react";
import { Button, Dialog, DialogActions, Input } from "@novamail/ui";

import { api, isDesktopShell } from "@/shared/api/client";
import type {
  AccountDto,
  AiRuntimeStatus,
  AiSettings,
  AppError,
  LabelDto,
  RuleDto,
  SignatureDto,
} from "@/shared/api/types";
import { useT } from "@/shared/i18n/useT";
import type { Locale } from "@/shared/i18n";
import type { ColorSchemeId } from "@/shared/theme/schemes";
import { useUiStore } from "@/shared/store/uiStore";

function downloadBase64File(filename: string, dataBase64: string) {
  const binary = atob(dataBase64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) {
    bytes[i] = binary.charCodeAt(i);
  }
  const blob = new Blob([bytes], { type: "application/octet-stream" });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = filename;
  anchor.click();
  URL.revokeObjectURL(url);
}

function fileToBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const result = reader.result;
      if (typeof result !== "string") {
        reject(new Error("read failed"));
        return;
      }
      const comma = result.indexOf(",");
      resolve(comma >= 0 ? result.slice(comma + 1) : result);
    };
    reader.onerror = () => reject(reader.error ?? new Error("read failed"));
    reader.readAsDataURL(file);
  });
}

export function SettingsDialog({
  open,
  onClose,
  accounts = [],
}: {
  open: boolean;
  onClose: () => void;
  accounts?: AccountDto[];
}) {
  const t = useT();
  const theme = useUiStore((s) => s.theme);
  const setTheme = useUiStore((s) => s.setTheme);
  const colorScheme = useUiStore((s) => s.colorScheme);
  const setColorScheme = useUiStore((s) => s.setColorScheme);
  const locale = useUiStore((s) => s.locale);
  const setLocale = useUiStore((s) => s.setLocale);
  const highContrast = useUiStore((s) => s.highContrast);
  const setHighContrast = useUiStore((s) => s.setHighContrast);
  const density = useUiStore((s) => s.density);
  const setDensity = useUiStore((s) => s.setDensity);

  const [signatures, setSignatures] = useState<SignatureDto[]>([]);
  const [labels, setLabels] = useState<LabelDto[]>([]);
  const [rules, setRules] = useState<RuleDto[]>([]);
  const [sigName, setSigName] = useState("");
  const [sigBody, setSigBody] = useState("");
  const [labelName, setLabelName] = useState("");
  const [ruleName, setRuleName] = useState("");
  const [ruleSubject, setRuleSubject] = useState("");
  const [backupPassphrase, setBackupPassphrase] = useState("");
  const [backupPassphraseConfirm, setBackupPassphraseConfirm] = useState("");
  const [backupBusy, setBackupBusy] = useState(false);
  const [backupStatus, setBackupStatus] = useState<string | null>(null);
  const [importFileName, setImportFileName] = useState<string | null>(null);
  const importFileRef = useRef<File | null>(null);
  const importInputRef = useRef<HTMLInputElement>(null);
  const [error, setError] = useState<string | null>(null);
  const [aiSettings, setAiSettings] = useState<AiSettings>({
    enabled: false,
    model: "qwen3:4b-instruct",
    baseUrl: "http://127.0.0.1:11434",
    onboardingCompleted: false,
  });
  const [aiRuntime, setAiRuntime] = useState<AiRuntimeStatus | null>(null);
  const [aiBusy, setAiBusy] = useState(false);
  const [aiStatus, setAiStatus] = useState<string | null>(null);

  async function refreshExtras() {
    const [sigs, labs, rls] = await Promise.all([
      api.signaturesList(null),
      api.labelsList(null),
      api.rulesList(),
    ]);
    setSignatures(sigs);
    setLabels(labs);
    setRules(rls);
  }

  async function refreshAi() {
    if (!isDesktopShell()) return;
    const [settings, runtime] = await Promise.all([
      api.aiGetSettings(),
      api.aiRuntimeStatus(),
    ]);
    setAiSettings(settings);
    setAiRuntime(runtime);
  }

  useEffect(() => {
    if (!open) return;
    setSigName(t("defaultSignatureName"));
    setBackupPassphrase("");
    setBackupPassphraseConfirm("");
    setBackupStatus(null);
    setAiStatus(null);
    setImportFileName(null);
    importFileRef.current = null;
    refreshExtras().catch((err) => setError((err as AppError).message));
    refreshAi().catch((err) => setError((err as AppError).message));
  }, [open, t]);

  async function handleSaveAi() {
    setError(null);
    setAiStatus(null);
    setAiBusy(true);
    try {
      const saved = await api.aiSetSettings({
        ...aiSettings,
        model: aiSettings.model.trim() || "qwen3:4b-instruct",
        baseUrl: aiSettings.baseUrl.trim() || "http://127.0.0.1:11434",
        onboardingCompleted: true,
      });
      setAiSettings(saved);
      setAiStatus(t("aiSettingsSaved"));
      await refreshAi();
    } catch (err) {
      setError((err as AppError).message);
    } finally {
      setAiBusy(false);
    }
  }

  async function handleExportBackup() {
    setError(null);
    setBackupStatus(null);
    if (backupPassphrase.trim().length < 8) {
      setError(t("backupPassphraseTooShort"));
      return;
    }
    if (backupPassphrase !== backupPassphraseConfirm) {
      setError(t("backupPassphraseMismatch"));
      return;
    }
    setBackupBusy(true);
    try {
      const result = await api.backupExport(backupPassphrase);
      downloadBase64File(result.filename, result.dataBase64);
      setBackupStatus(
        t("backupExportDone", {
          accounts: result.accounts,
          contacts: result.contacts,
        }),
      );
      setBackupPassphrase("");
      setBackupPassphraseConfirm("");
    } catch (err) {
      setError((err as AppError).message || t("backupFailed"));
    } finally {
      setBackupBusy(false);
    }
  }

  async function handleImportBackup() {
    setError(null);
    setBackupStatus(null);
    if (backupPassphrase.trim().length < 8) {
      setError(t("backupPassphraseTooShort"));
      return;
    }
    const file = importFileRef.current;
    if (!file) {
      setError(t("backupNoFile"));
      return;
    }
    setBackupBusy(true);
    try {
      const dataBase64 = await fileToBase64(file);
      const result = await api.backupImport(backupPassphrase, dataBase64);
      setBackupStatus(
        t("backupImportDone", {
          accounts: result.accountsImported + result.accountsUpdated,
          contacts: result.contactsImported + result.contactsUpdated,
          skipped: result.contactsSkipped,
        }),
      );
      setBackupPassphrase("");
      setBackupPassphraseConfirm("");
      setImportFileName(null);
      importFileRef.current = null;
      if (importInputRef.current) importInputRef.current.value = "";
      await refreshExtras();
    } catch (err) {
      const appErr = err as AppError;
      if (appErr.code === "bad_passphrase") {
        setError(t("backupBadPassphrase"));
      } else {
        setError(appErr.message || t("backupFailed"));
      }
    } finally {
      setBackupBusy(false);
    }
  }

  return (
    <Dialog
      open={open}
      onClose={onClose}
      title={t("settingsTitle")}
      description={t("settingsDescription")}
    >
      <div className="grid max-h-[70vh] gap-5 overflow-y-auto text-sm">
        <label className="grid gap-1">
          <span>{t("language")}</span>
          <select
            className="h-11 rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] bg-[var(--nova-surface)] px-3"
            value={locale}
            onChange={(e) => setLocale(e.target.value as Locale)}
          >
            <option value="de">{t("languageGerman")}</option>
            <option value="en">{t("languageEnglish")}</option>
          </select>
        </label>

        <label className="grid gap-1">
          <span>{t("colorScheme")}</span>
          <select
            className="h-11 rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] bg-[var(--nova-surface)] px-3"
            value={colorScheme}
            onChange={(e) => setColorScheme(e.target.value as ColorSchemeId)}
          >
            <option value="navy">{t("schemeNavy")}</option>
            <option value="forest">{t("schemeForest")}</option>
            <option value="slate">{t("schemeSlate")}</option>
            <option value="midnight">{t("schemeMidnight")}</option>
          </select>
        </label>

        <div className="grid grid-cols-4 gap-2" aria-hidden>
          {(
            [
              ["navy", "#1e3a5f", "#d7e4f4"],
              ["forest", "#0b6e4f", "#d8f3e7"],
              ["slate", "#334155", "#e2e8f0"],
              ["midnight", "#312e81", "#e0e7ff"],
            ] as const
          ).map(([id, accent, soft]) => {
            const selected = colorScheme === id;
            return (
              <div
                key={id}
                className={
                  selected
                    ? "rounded-[8px] p-[3px] ring-2 ring-inset ring-[var(--nova-accent)]"
                    : "rounded-[8px] p-[3px] ring-2 ring-inset ring-transparent"
                }
              >
                <button
                  type="button"
                  title={id}
                  onClick={() => setColorScheme(id)}
                  className="h-6 w-full rounded-[5px] border border-[var(--nova-border)]"
                  style={{
                    background: `linear-gradient(135deg, ${soft} 55%, ${accent})`,
                  }}
                />
              </div>
            );
          })}
        </div>

        <label className="grid gap-1">
          <span>{t("theme")}</span>
          <select
            className="h-11 rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] bg-[var(--nova-surface)] px-3"
            value={theme}
            onChange={(e) => setTheme(e.target.value as "light" | "dark" | "system")}
          >
            <option value="system">{t("themeSystem")}</option>
            <option value="light">{t("themeLight")}</option>
            <option value="dark">{t("themeDark")}</option>
          </select>
          <span className="text-xs text-[var(--nova-ink-muted)]">
            {t("themeSystemHint")}
          </span>
        </label>
        <label className="grid gap-1">
          <span>{t("density")}</span>
          <select
            className="h-11 rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] bg-[var(--nova-surface)] px-3"
            value={density}
            onChange={(e) =>
              setDensity(e.target.value as "comfortable" | "compact")
            }
          >
            <option value="comfortable">{t("densityComfortable")}</option>
            <option value="compact">{t("densityCompact")}</option>
          </select>
        </label>
        <label className="flex items-center gap-2">
          <input
            type="checkbox"
            checked={highContrast}
            onChange={(e) => setHighContrast(e.target.checked)}
          />
          {t("highContrast")}
        </label>

        <section className="grid gap-2">
          <h3 className="font-medium">{t("aiSettingsTitle")}</h3>
          <p className="text-xs text-[var(--nova-ink-muted)]">
            {t("aiSettingsDescription")}
          </p>
          <label className="flex items-center gap-2 text-sm">
            <input
              type="checkbox"
              checked={aiSettings.enabled}
              onChange={(e) =>
                setAiSettings((prev) => ({ ...prev, enabled: e.target.checked }))
              }
            />
            {t("aiSettingsEnabled")}
          </label>
          <p className="text-xs text-[var(--nova-ink-muted)]">
            {aiRuntime?.nvidiaGpu ? t("aiRuntimeGpuYes") : t("aiRuntimeGpuNo")}
            {" · "}
            {aiRuntime?.ollamaReachable
              ? t("aiRuntimeOllamaYes")
              : t("aiRuntimeOllamaNo")}
          </p>
          <label className="grid gap-1 text-sm">
            <span>{t("aiSettingsModel")}</span>
            {aiRuntime && aiRuntime.models.length > 0 ? (
              <select
                className="h-11 rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] bg-[var(--nova-surface)] px-3"
                value={aiSettings.model}
                disabled={!aiSettings.enabled}
                onChange={(e) =>
                  setAiSettings((prev) => ({ ...prev, model: e.target.value }))
                }
              >
                {Array.from(
                  new Set([
                    "qwen3:4b-instruct",
                    "qwen2.5:1.5b",
                    ...aiRuntime.models,
                    aiSettings.model,
                    aiRuntime.recommendedModel,
                  ]),
                ).map((name) => (
                  <option key={name} value={name}>
                    {name === "qwen3:4b-instruct"
                      ? `${name} (${t("aiSetupRecommended")})`
                      : name === "qwen2.5:1.5b"
                        ? `${name} (${t("aiSetupLowSpec")})`
                        : name === aiRuntime.recommendedModel
                          ? `${name} (${t("aiSetupRecommended")})`
                          : name}
                  </option>
                ))}
              </select>
            ) : (
              <Input
                value={aiSettings.model}
                disabled={!aiSettings.enabled}
                onChange={(e) =>
                  setAiSettings((prev) => ({ ...prev, model: e.target.value }))
                }
              />
            )}
          </label>
          <label className="grid gap-1 text-sm">
            <span>{t("aiSettingsBaseUrl")}</span>
            <Input
              value={aiSettings.baseUrl}
              disabled={!aiSettings.enabled}
              onChange={(e) =>
                setAiSettings((prev) => ({ ...prev, baseUrl: e.target.value }))
              }
            />
          </label>
          <div className="flex flex-wrap gap-2">
            <Button
              type="button"
              size="sm"
              disabled={aiBusy}
              onClick={() => void handleSaveAi()}
            >
              {aiBusy ? t("working") : t("aiSettingsSave")}
            </Button>
            <Button
              type="button"
              size="sm"
              variant="ghost"
              disabled={aiBusy}
              onClick={() => {
                void refreshAi()
                  .then(() => setAiStatus(null))
                  .catch((err) => setError((err as AppError).message));
              }}
            >
              {t("aiSettingsRefresh")}
            </Button>
          </div>
          {aiStatus ? (
            <p className="text-xs text-[var(--nova-ink-muted)]">{aiStatus}</p>
          ) : null}
        </section>

        <section className="grid gap-2">
          <h3 className="font-medium">{t("backupTitle")}</h3>
          <p className="text-xs text-[var(--nova-ink-muted)]">
            {t("backupDescription")}
          </p>
          <Input
            type="password"
            autoComplete="new-password"
            placeholder={t("backupPassphrase")}
            value={backupPassphrase}
            onChange={(e) => setBackupPassphrase(e.target.value)}
          />
          <Input
            type="password"
            autoComplete="new-password"
            placeholder={t("backupPassphraseConfirm")}
            value={backupPassphraseConfirm}
            onChange={(e) => setBackupPassphraseConfirm(e.target.value)}
          />
          <div className="flex flex-wrap gap-2">
            <Button
              type="button"
              size="sm"
              disabled={backupBusy}
              onClick={() => void handleExportBackup()}
            >
              {t("backupExport")}
            </Button>
            <Button
              type="button"
              size="sm"
              variant="ghost"
              disabled={backupBusy}
              onClick={() => importInputRef.current?.click()}
            >
              {t("backupChooseFile")}
            </Button>
            <Button
              type="button"
              size="sm"
              disabled={backupBusy || !importFileName}
              onClick={() => void handleImportBackup()}
            >
              {t("backupImport")}
            </Button>
          </div>
          <input
            ref={importInputRef}
            type="file"
            accept=".nmbak,application/json,application/octet-stream"
            className="hidden"
            onChange={(e) => {
              const file = e.target.files?.[0] ?? null;
              importFileRef.current = file;
              setImportFileName(file?.name ?? null);
              setBackupStatus(null);
              setError(null);
            }}
          />
          {importFileName ? (
            <p className="text-xs text-[var(--nova-ink-muted)]">{importFileName}</p>
          ) : null}
          {backupStatus ? (
            <p className="text-xs text-[var(--nova-ink-muted)]" role="status">
              {backupStatus}
            </p>
          ) : null}
        </section>

        <section className="grid gap-2">
          <h3 className="font-medium">{t("signatures")}</h3>
          <Input value={sigName} onChange={(e) => setSigName(e.target.value)} />
          <textarea
            className="min-h-[80px] rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] bg-[var(--nova-surface)] px-3 py-2"
            value={sigBody}
            onChange={(e) => setSigBody(e.target.value)}
            placeholder={t("signatureText")}
          />
          <Button
            type="button"
            size="sm"
            onClick={async () => {
              await api.signaturesUpsert({
                name: sigName,
                bodyText: sigBody,
                isDefault: true,
                accountId: accounts[0]?.id ?? null,
              });
              setSigBody("");
              await refreshExtras();
            }}
          >
            {t("saveSignature")}
          </Button>
          <ul className="space-y-1 text-[var(--nova-ink-muted)]">
            {signatures.map((sig) => (
              <li key={sig.id}>
                {sig.name}
                {sig.isDefault ? ` ${t("defaultSuffix")}` : ""}
              </li>
            ))}
          </ul>
        </section>

        <section className="grid gap-2">
          <h3 className="font-medium">{t("labels")}</h3>
          <Input
            placeholder={t("labelName")}
            value={labelName}
            onChange={(e) => setLabelName(e.target.value)}
          />
          <Button
            type="button"
            size="sm"
            disabled={!accounts[0]}
            onClick={async () => {
              if (!accounts[0] || !labelName.trim()) return;
              await api.labelsUpsert({
                accountId: accounts[0].id,
                name: labelName.trim(),
                color: "#1e3a5f",
              });
              setLabelName("");
              await refreshExtras();
            }}
          >
            {t("addLabel")}
          </Button>
          <ul className="space-y-1 text-[var(--nova-ink-muted)]">
            {labels.map((label) => (
              <li key={label.id}>{label.name}</li>
            ))}
          </ul>
        </section>

        <section className="grid gap-2">
          <h3 className="font-medium">{t("rules")}</h3>
          <Input
            placeholder={t("ruleName")}
            value={ruleName}
            onChange={(e) => setRuleName(e.target.value)}
          />
          <Input
            placeholder={t("subjectContains")}
            value={ruleSubject}
            onChange={(e) => setRuleSubject(e.target.value)}
          />
          <Button
            type="button"
            size="sm"
            onClick={async () => {
              if (!ruleName.trim() || !ruleSubject.trim()) return;
              await api.rulesUpsert({
                name: ruleName.trim(),
                enabled: true,
                accountId: null,
                predicateJson: JSON.stringify({
                  type: "subjectContains",
                  value: ruleSubject.trim(),
                }),
                actionJson: JSON.stringify([{ type: "markRead" }]),
              });
              setRuleName("");
              setRuleSubject("");
              await refreshExtras();
            }}
          >
            {t("addRule")}
          </Button>
          <ul className="space-y-1 text-[var(--nova-ink-muted)]">
            {rules.map((rule) => (
              <li key={rule.id}>{rule.name}</li>
            ))}
          </ul>
        </section>

        {error ? (
          <p className="text-[var(--nova-danger)]" role="alert">
            {error}
          </p>
        ) : null}
      </div>
      <DialogActions>
        <Button onClick={onClose}>{t("done")}</Button>
      </DialogActions>
    </Dialog>
  );
}
