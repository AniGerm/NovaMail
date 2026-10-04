import { useEffect, useRef, useState } from "react";
import { Button, Dialog, DialogActions, Input, Select } from "@novamail/ui";

import { api, isDesktopShell } from "@/shared/api/client";
import type {
  AccountDto,
  AiRuntimeStatus,
  AiSettings,
  AppError,
  LabelDto,
  SignatureDto,
  SpellDictionaryDto,
  UpdateStatusEvent,
} from "@/shared/api/types";
import { ContactsSettingsPanel } from "@/features/contacts/ContactsSettingsPanel";
import { AccountsPanel } from "@/features/settings/AccountsPanel";
import { OfflineMailboxPanel } from "@/features/settings/OfflineMailboxPanel";
import { PgpKeysPanel } from "@/features/settings/PgpKeysPanel";
import { RulesSpamPanel } from "@/features/settings/RulesSpamPanel";

const AI_DEFAULT_MODEL = "qwen3:4b-instruct";
const AI_CPU_MODEL = "qwen2.5:1.5b";
/** Always-visible spellcheck languages (ship with app / launcher). */
const PRIMARY_SPELL_CODES = new Set(["de_DE", "en_US"]);
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

type UpdateUiState = {
  status: string;
  availableVersion: string | null;
  downloadedVersion: string | null;
  progress: number | null;
  checking: boolean;
  downloading: boolean;
};

const INITIAL_UPDATE: UpdateUiState = {
  status: "",
  availableVersion: null,
  downloadedVersion: null,
  progress: null,
  checking: false,
  downloading: false,
};

function applyUpdateEvent(
  prev: UpdateUiState,
  event: UpdateStatusEvent,
  labels: {
    checking: string;
    available: (version: string) => string;
    notAvailable: string;
    downloading: (percent: number) => string;
    downloaded: (version: string) => string;
    installing: string;
  },
): UpdateUiState {
  switch (event.type) {
    case "checking":
      return { ...prev, checking: true, status: labels.checking };
    case "update-available":
      return {
        ...prev,
        checking: false,
        availableVersion: event.version,
        downloadedVersion: null,
        progress: null,
        status: labels.available(event.version),
      };
    case "update-not-available":
      return {
        ...prev,
        checking: false,
        availableVersion: null,
        status: labels.notAvailable,
      };
    case "download-progress":
      return {
        ...prev,
        downloading: true,
        progress: event.percent,
        status: labels.downloading(event.percent),
      };
    case "update-downloaded":
      return {
        ...prev,
        checking: false,
        downloading: false,
        progress: 100,
        availableVersion: event.version,
        downloadedVersion: event.version,
        status: labels.downloaded(event.version),
      };
    case "installing":
      return {
        ...prev,
        checking: false,
        downloading: false,
        status: labels.installing,
      };
    case "error":
      return {
        ...prev,
        checking: false,
        downloading: false,
        status: event.message,
      };
    default:
      return prev;
  }
}

type SettingsTab =
  | "general"
  | "accounts"
  | "ai"
  | "mail"
  | "privacy"
  | "contacts";

export function SettingsDialog({
  open,
  onClose,
  accounts = [],
  onAccountsChanged,
  onAddAccount,
  initialTab = "general",
}: {
  open: boolean;
  onClose: () => void;
  accounts?: AccountDto[];
  onAccountsChanged?: () => void;
  onAddAccount?: () => void;
  initialTab?: SettingsTab;
}) {
  const t = useT();
  const theme = useUiStore((s) => s.theme);
  const setTheme = useUiStore((s) => s.setTheme);
  const colorScheme = useUiStore((s) => s.colorScheme);
  const setColorScheme = useUiStore((s) => s.setColorScheme);
  const locale = useUiStore((s) => s.locale);
  const setLocale = useUiStore((s) => s.setLocale);
  const spellcheckLang = useUiStore((s) => s.spellcheckLang);
  const setSpellcheckLang = useUiStore((s) => s.setSpellcheckLang);
  const highContrast = useUiStore((s) => s.highContrast);
  const setHighContrast = useUiStore((s) => s.setHighContrast);
  const density = useUiStore((s) => s.density);
  const setDensity = useUiStore((s) => s.setDensity);
  const autoCheckUpdates = useUiStore((s) => s.autoCheckUpdates);
  const setAutoCheckUpdates = useUiStore((s) => s.setAutoCheckUpdates);

  const [signatures, setSignatures] = useState<SignatureDto[]>([]);
  const [labels, setLabels] = useState<LabelDto[]>([]);
  const [sigName, setSigName] = useState("");
  const [sigBody, setSigBody] = useState("");
  const [labelName, setLabelName] = useState("");
  const [backupPassphrase, setBackupPassphrase] = useState("");
  const [backupPassphraseConfirm, setBackupPassphraseConfirm] = useState("");
  const [backupBusy, setBackupBusy] = useState(false);
  const [backupStatus, setBackupStatus] = useState<string | null>(null);
  const [importFileName, setImportFileName] = useState<string | null>(null);
  const importFileRef = useRef<File | null>(null);
  const importInputRef = useRef<HTMLInputElement>(null);
  const [error, setError] = useState<string | null>(null);
  const [appVersion, setAppVersion] = useState("");
  const [logsPath, setLogsPath] = useState("");
  const [updateChannel, setUpdateChannel] = useState("dev");
  const [updateUi, setUpdateUi] = useState<UpdateUiState>(INITIAL_UPDATE);
  const [aiSettings, setAiSettings] = useState<AiSettings>({
    enabled: false,
    model: AI_DEFAULT_MODEL,
    baseUrl: "http://127.0.0.1:11434",
    onboardingCompleted: false,
  });
  const [aiRuntime, setAiRuntime] = useState<AiRuntimeStatus | null>(null);
  const [aiBusy, setAiBusy] = useState(false);
  const [aiStatus, setAiStatus] = useState<string | null>(null);
  const [pullRef, setPullRef] = useState("");
  const [pullBusy, setPullBusy] = useState(false);
  const [pullStatus, setPullStatus] = useState<string | null>(null);
  const [spellDicts, setSpellDicts] = useState<SpellDictionaryDto[]>([]);
  const [spellBusyCode, setSpellBusyCode] = useState<string | null>(null);
  const [spellStatus, setSpellStatus] = useState<string | null>(null);
  const [otherLangsOpen, setOtherLangsOpen] = useState(false);
  const [closeToTray, setCloseToTray] = useState(true);
  const [autostart, setAutostart] = useState(false);
  const [settingsTab, setSettingsTab] = useState<SettingsTab>("general");
  const primarySpellDicts = spellDicts.filter((d) =>
    PRIMARY_SPELL_CODES.has(d.code),
  );
  const otherSpellDicts = spellDicts.filter(
    (d) => !PRIMARY_SPELL_CODES.has(d.code),
  );
  const shouldRevealOtherLangs =
    (!PRIMARY_SPELL_CODES.has(spellcheckLang) &&
      otherSpellDicts.some((d) => d.code === spellcheckLang)) ||
    (spellBusyCode != null && !PRIMARY_SPELL_CODES.has(spellBusyCode));

  useEffect(() => {
    if (shouldRevealOtherLangs) setOtherLangsOpen(true);
  }, [shouldRevealOtherLangs]);

  async function refreshSpellcheck() {
    if (!isDesktopShell()) return;
    try {
      const status = await api.spellcheckStatus();
      setSpellDicts(status.dictionaries);
    } catch {
      /* ignore when shell APIs unavailable */
    }
  }

  async function refreshShellPrefs() {
    if (!isDesktopShell()) return;
    try {
      const prefs = await api.shellGetPrefs();
      setCloseToTray(prefs.closeToTray);
      setAutostart(prefs.autostart);
    } catch {
      /* ignore when shell APIs unavailable */
    }
  }

  async function refreshExtras() {
    const [sigs, labs] = await Promise.all([
      api.signaturesList(null),
      api.labelsList(null),
    ]);
    setSignatures(sigs);
    setLabels(labs);
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
    if (open) setSettingsTab(initialTab);
  }, [open, initialTab]);

  useEffect(() => {
    if (!open) return;
    setSigName(t("defaultSignatureName"));
    setBackupPassphrase("");
    setBackupPassphraseConfirm("");
    setBackupStatus(null);
    setAiStatus(null);
    setSpellStatus(null);
    setImportFileName(null);
    importFileRef.current = null;
    refreshExtras().catch((err) => setError((err as AppError).message));
    refreshAi().catch((err) => setError((err as AppError).message));
    refreshSpellcheck().catch(() => undefined);
    refreshShellPrefs().catch(() => undefined);
    if (isDesktopShell()) {
      void api
        .appVersion()
        .then((info) => {
          setAppVersion(info.version);
          setUpdateChannel(info.channel);
        })
        .catch(() => undefined);
      void api
        .logsPath()
        .then((path) => setLogsPath(path))
        .catch(() => undefined);
    }
  }, [open, t]);

  useEffect(() => {
    if (!open || !isDesktopShell()) return;
    let unlisten: (() => void) | undefined;
    void api
      .onUpdateStatus((event) => {
        setUpdateUi((prev) =>
          applyUpdateEvent(prev, event, {
            checking: t("updatesChecking"),
            available: (version) => t("updatesAvailable", { version }),
            notAvailable: t("updatesNotAvailable"),
            downloading: (percent) => t("updatesDownloading", { percent }),
            downloaded: (version) => t("updatesDownloaded", { version }),
            installing: t("updatesInstalling"),
          }),
        );
      })
      .then((fn) => {
        unlisten = fn;
      });
    return () => {
      unlisten?.();
    };
  }, [open, t]);

  async function handleSaveAi() {
    setError(null);
    setAiStatus(null);
    setAiBusy(true);
    try {
      const saved = await api.aiSetSettings({
        ...aiSettings,
        model: aiSettings.model.trim() || AI_DEFAULT_MODEL,
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

  async function handlePullModel(raw?: string) {
    const target = (raw ?? pullRef).trim();
    if (!target) {
      setError(t("aiPullFailed"));
      return;
    }
    setError(null);
    setPullBusy(true);
    setPullStatus(t("aiPullBusy"));
    let unlisten: (() => void) | undefined;
    try {
      unlisten = await api.onAiPullProgress((event) => {
        if (event.total && event.completed != null) {
          const pct = Math.min(
            100,
            Math.round((event.completed / event.total) * 100),
          );
          setPullStatus(`${event.status} · ${pct}%`);
        } else {
          setPullStatus(event.status);
        }
      });
      const result = await api.aiPullModel(target);
      setPullRef(result.model);
      setAiSettings((prev) => ({ ...prev, model: result.model, enabled: true }));
      setPullStatus(t("aiPullDone", { model: result.model }));
      await refreshAi();
    } catch (err) {
      setError((err as AppError).message || t("aiPullFailed"));
      setPullStatus(null);
    } finally {
      unlisten?.();
      setPullBusy(false);
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

  async function handleCheckUpdates() {
    if (!isDesktopShell()) return;
    setUpdateUi((prev) => ({
      ...prev,
      checking: true,
      status: t("updatesChecking"),
    }));
    try {
      await api.updatesCheck();
    } catch (err) {
      setUpdateUi((prev) => ({
        ...prev,
        checking: false,
        downloading: false,
        status: (err as AppError).message,
      }));
    }
  }

  async function handleDownloadUpdate() {
    if (!isDesktopShell()) return;
    setUpdateUi((prev) => ({ ...prev, downloading: true, progress: 0 }));
    try {
      await api.updatesDownload();
    } catch (err) {
      setUpdateUi((prev) => ({
        ...prev,
        checking: false,
        downloading: false,
        status: (err as AppError).message,
      }));
    }
  }

  async function handleInstallUpdate() {
    if (!isDesktopShell()) return;
    try {
      await api.updatesInstall();
    } catch (err) {
      setUpdateUi((prev) => ({
        ...prev,
        checking: false,
        downloading: false,
        status: (err as AppError).message,
      }));
    }
  }

  return (
    <Dialog
      open={open}
      onClose={onClose}
      title={t("settingsTitle")}
      description={t("settingsDescription")}
      className="h-[min(100%,720px)] w-[min(960px,100%)] max-w-5xl"
    >
      <div className="flex h-full min-h-0 flex-1 flex-col">
      <div className="flex shrink-0 flex-wrap gap-1 border-b border-[var(--nova-border)] pb-2">
        {(
          [
            ["general", t("settingsTabGeneral")],
            ["accounts", t("settingsTabAccounts")],
            ["contacts", t("settingsTabContacts")],
            ["ai", t("settingsTabAi")],
            ["mail", t("settingsTabMail")],
            ["privacy", t("settingsTabPrivacy")],
          ] as const
        ).map(([id, label]) => (
          <button
            key={id}
            type="button"
            onClick={() => setSettingsTab(id)}
            className={
              settingsTab === id
                ? "rounded-[var(--nova-radius-sm)] bg-[var(--nova-accent-soft)] px-3 py-1.5 text-sm font-medium text-[var(--nova-accent)]"
                : "rounded-[var(--nova-radius-sm)] px-3 py-1.5 text-sm text-[var(--nova-ink-muted)] hover:bg-[var(--nova-surface-2)]"
            }
          >
            {label}
          </button>
        ))}
      </div>
      <div className="grid min-h-0 flex-1 gap-5 overflow-y-auto pt-4 text-sm">
        {settingsTab === "contacts" ? <ContactsSettingsPanel /> : null}
        {settingsTab === "accounts" ? (
          <AccountsPanel
            accounts={accounts}
            onChanged={() => onAccountsChanged?.()}
            onAddAccount={() => onAddAccount?.()}
          />
        ) : null}
        {settingsTab === "general" ? (
        <>

        <label className="grid gap-1">
          <span>{t("language")}</span>
          <Select
            className="h-11"
            value={locale}
            onChange={(e) => {
              const next = e.target.value as Locale;
              setLocale(next);
              if (isDesktopShell()) {
                void api.shellSetUiLocale(next).catch(() => undefined);
                void api
                  .spellcheckEnsureForLocale(next)
                  .then(() => refreshSpellcheck())
                  .catch((err) => setError((err as AppError).message));
              }
            }}
          >
            <option value="de">{t("languageGerman")}</option>
            <option value="en">{t("languageEnglish")}</option>
          </Select>
        </label>

        <section className="grid gap-2 rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] p-3">
          <h3 className="font-medium">{t("spellcheckTitle")}</h3>
          <p className="text-xs text-[var(--nova-ink-muted)]">
            {t("spellcheckDescription")}
          </p>
          <label className="grid gap-1">
            <span>{t("spellcheckActive")}</span>
            <Select
              className="h-11"
              value={spellcheckLang}
              onChange={(e) => {
                const code = e.target.value;
                setSpellcheckLang(code);
                const short = code.split("_")[0] ?? code;
                if (isDesktopShell()) {
                  void api
                    .spellcheckSetLanguages([code, short])
                    .catch(() => undefined);
                }
              }}
            >
              {spellDicts.map((dict) => (
                <option key={dict.code} value={dict.code} disabled={!dict.installed}>
                  {dict.name}
                  {dict.installed ? "" : ` (${t("spellcheckMissing")})`}
                </option>
              ))}
            </Select>
          </label>
          <ul className="grid gap-1.5">
            {primarySpellDicts.map((dict) => (
              <SpellDictRow
                key={dict.code}
                dict={dict}
                activeCode={spellcheckLang}
                busyCode={spellBusyCode}
                onUse={setSpellcheckLang}
                onInstall={(code) => {
                  setSpellBusyCode(code);
                  setSpellStatus(null);
                  setError(null);
                  void api
                    .spellcheckInstall(code)
                    .then((installed) => {
                      setSpellcheckLang(installed.code);
                      setSpellStatus(
                        t("spellcheckInstallOk", { name: installed.name }),
                      );
                      return refreshSpellcheck();
                    })
                    .catch((err) => setError((err as AppError).message))
                    .finally(() => setSpellBusyCode(null));
                }}
              />
            ))}
          </ul>
          {otherSpellDicts.length > 0 ? (
            <details
              className="rounded-[var(--nova-radius-sm)] border border-[var(--nova-border)] open:bg-[color-mix(in_srgb,var(--nova-surface-2)_55%,transparent)]"
              open={otherLangsOpen}
              onToggle={(e) => setOtherLangsOpen(e.currentTarget.open)}
            >
              <summary className="cursor-pointer select-none px-2 py-2 font-medium marker:text-[var(--nova-ink-muted)]">
                {t("spellcheckOtherLanguages")}
                <span className="mt-0.5 block text-xs font-normal text-[var(--nova-ink-muted)]">
                  {t("spellcheckOtherLanguagesHint")}
                </span>
              </summary>
              <ul className="grid gap-1.5 px-2 pb-2">
                {otherSpellDicts.map((dict) => (
                  <SpellDictRow
                    key={dict.code}
                    dict={dict}
                    activeCode={spellcheckLang}
                    busyCode={spellBusyCode}
                    onUse={setSpellcheckLang}
                    onInstall={(code) => {
                      setSpellBusyCode(code);
                      setSpellStatus(null);
                      setError(null);
                      void api
                        .spellcheckInstall(code)
                        .then((installed) => {
                          setSpellcheckLang(installed.code);
                          setSpellStatus(
                            t("spellcheckInstallOk", {
                              name: installed.name,
                            }),
                          );
                          return refreshSpellcheck();
                        })
                        .catch((err) => setError((err as AppError).message))
                        .finally(() => setSpellBusyCode(null));
                    }}
                  />
                ))}
              </ul>
            </details>
          ) : null}
          <p className="text-xs text-[var(--nova-ink-muted)]">
            {t("spellcheckEnsureHint")}
          </p>
          {spellStatus ? (
            <p className="text-xs text-[var(--nova-accent)]" role="status">
              {spellStatus}
            </p>
          ) : null}
        </section>

        <label className="grid gap-1">
          <span>{t("colorScheme")}</span>
          <Select
            className="h-11"
            value={colorScheme}
            onChange={(e) => setColorScheme(e.target.value as ColorSchemeId)}
          >
            <option value="navy">{t("schemeNavy")}</option>
            <option value="forest">{t("schemeForest")}</option>
            <option value="slate">{t("schemeSlate")}</option>
            <option value="midnight">{t("schemeMidnight")}</option>
          </Select>
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
          <Select
            className="h-11"
            value={theme}
            onChange={(e) => setTheme(e.target.value as "light" | "dark" | "system")}
          >
            <option value="system">{t("themeSystem")}</option>
            <option value="light">{t("themeLight")}</option>
            <option value="dark">{t("themeDark")}</option>
          </Select>
          <span className="text-xs text-[var(--nova-ink-muted)]">
            {t("themeSystemHint")}
          </span>
        </label>
        <label className="grid gap-1">
          <span>{t("density")}</span>
          <Select
            className="h-11"
            value={density}
            onChange={(e) =>
              setDensity(e.target.value as "comfortable" | "compact")
            }
          >
            <option value="comfortable">{t("densityComfortable")}</option>
            <option value="compact">{t("densityCompact")}</option>
          </Select>
        </label>
        <label className="flex items-center gap-2">
          <input
            type="checkbox"
            checked={highContrast}
            onChange={(e) => setHighContrast(e.target.checked)}
          />
          {t("highContrast")}
        </label>

        {isDesktopShell() ? (
          <section className="grid gap-3 rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] p-3">
            <label className="grid gap-1">
              <span className="flex items-center gap-2">
                <input
                  type="checkbox"
                  checked={closeToTray}
                  onChange={(e) => {
                    const enabled = e.target.checked;
                    setCloseToTray(enabled);
                    void api
                      .shellSetCloseToTray(enabled)
                      .then((prefs) => {
                        setCloseToTray(prefs.closeToTray);
                        setAutostart(prefs.autostart);
                      })
                      .catch((err) => setError((err as AppError).message));
                  }}
                />
                {t("closeToTray")}
              </span>
              <span className="text-xs text-[var(--nova-ink-muted)]">
                {t("closeToTrayHint")}
              </span>
            </label>
            <label className="grid gap-1">
              <span className="flex items-center gap-2">
                <input
                  type="checkbox"
                  checked={autostart}
                  onChange={(e) => {
                    const enabled = e.target.checked;
                    setAutostart(enabled);
                    void api
                      .shellSetAutostart(enabled)
                      .then((prefs) => {
                        setCloseToTray(prefs.closeToTray);
                        setAutostart(prefs.autostart);
                      })
                      .catch((err) => setError((err as AppError).message));
                  }}
                />
                {t("autostart")}
              </span>
              <span className="text-xs text-[var(--nova-ink-muted)]">
                {t("autostartHint")}
              </span>
            </label>
          </section>
        ) : null}

        <section className="grid gap-2 rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] p-3">
          <h3 className="font-medium">{t("updatesTitle")}</h3>
          <p className="text-xs text-[var(--nova-ink-muted)]">
            {t("updatesCurrentVersion", {
              version: appVersion || "…",
            })}
            {" — "}
            {updateChannel === "deb"
              ? t("updatesDescriptionDeb")
              : t("updatesDescriptionDev")}
          </p>
          <label className="flex items-center gap-2">
            <input
              type="checkbox"
              checked={autoCheckUpdates}
              onChange={(e) => setAutoCheckUpdates(e.target.checked)}
            />
            {t("updatesAutoCheck")}
          </label>
          <div className="flex flex-wrap gap-2">
            <Button
              type="button"
              variant="secondary"
              disabled={
                !isDesktopShell() || updateUi.checking || updateUi.downloading
              }
              onClick={() => void handleCheckUpdates()}
            >
              {t("updatesCheck")}
            </Button>
            {updateUi.availableVersion && !updateUi.downloadedVersion ? (
              <Button
                type="button"
                disabled={updateUi.downloading}
                onClick={() => void handleDownloadUpdate()}
              >
                {t("updatesDownload")}
              </Button>
            ) : null}
            {updateUi.downloadedVersion ? (
              <Button type="button" onClick={() => void handleInstallUpdate()}>
                {t("updatesInstall")}
              </Button>
            ) : null}
          </div>
          {updateUi.progress !== null && updateUi.downloading ? (
            <div
              className="h-2 overflow-hidden rounded-full bg-[var(--nova-border)]"
              role="progressbar"
              aria-valuenow={updateUi.progress}
              aria-valuemin={0}
              aria-valuemax={100}
            >
              <div
                className="h-full bg-[var(--nova-accent)] transition-[width]"
                style={{ width: `${updateUi.progress}%` }}
              />
            </div>
          ) : null}
          {updateUi.status ? (
            <p className="text-xs text-[var(--nova-ink-muted)]">{updateUi.status}</p>
          ) : null}
        </section>

        {logsPath ? (
          <section className="grid gap-1 rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] p-3">
            <h3 className="font-medium">{t("crashLogTitle")}</h3>
            <p className="text-xs text-[var(--nova-ink-muted)]">
              {t("crashLogHint")}
            </p>
            <code className="break-all rounded bg-[var(--nova-surface-2)] px-2 py-1 text-xs">
              {logsPath}
            </code>
          </section>
        ) : null}

        </>
        ) : null}
        {settingsTab === "ai" ? (
        <>
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
              : aiRuntime?.ollamaInstalled
                ? t("aiInstallFound")
                : t("aiRuntimeOllamaNo")}
          </p>
          {aiRuntime && !aiRuntime.ollamaReachable ? (
            <div className="flex flex-wrap gap-2">
              {aiRuntime.ollamaInstalled ? (
                <Button
                  type="button"
                  size="sm"
                  disabled={pullBusy || aiBusy}
                  onClick={() => {
                    void api
                      .aiStartOllama()
                      .then(() => refreshAi())
                      .then(() => setAiStatus(t("aiInstallReady")))
                      .catch((err) => setError((err as AppError).message));
                  }}
                >
                  {t("aiInstallStart")}
                </Button>
              ) : null}
              {aiRuntime.canInstallUser ? (
                <Button
                  type="button"
                  size="sm"
                  variant="secondary"
                  disabled={pullBusy || aiBusy}
                  onClick={() => {
                    setAiBusy(true);
                    void api
                      .aiInstallOllama("user")
                      .then(() => refreshAi())
                      .then(() => setAiStatus(t("aiInstallReady")))
                      .catch((err) => setError((err as AppError).message))
                      .finally(() => setAiBusy(false));
                  }}
                >
                  {t("aiInstallUser")}
                </Button>
              ) : null}
              {aiRuntime.canInstallSystem ? (
                <Button
                  type="button"
                  size="sm"
                  variant="ghost"
                  disabled={pullBusy || aiBusy}
                  onClick={() => {
                    setAiBusy(true);
                    void api
                      .aiInstallOllama("system")
                      .then(() => refreshAi())
                      .then(() => setAiStatus(t("aiInstallReady")))
                      .catch((err) => setError((err as AppError).message))
                      .finally(() => setAiBusy(false));
                  }}
                >
                  {t("aiInstallSystem")}
                </Button>
              ) : null}
            </div>
          ) : null}
          <label className="grid gap-1 text-sm">
            <span>{t("aiSettingsModel")}</span>
            {aiRuntime && aiRuntime.models.length > 0 ? (
              <Select
                className="h-11"
                value={aiSettings.model}
                disabled={!aiSettings.enabled}
                onChange={(e) =>
                  setAiSettings((prev) => ({ ...prev, model: e.target.value }))
                }
              >
                {Array.from(
                  new Set([
                    AI_DEFAULT_MODEL,
                    AI_CPU_MODEL,
                    ...aiRuntime.models,
                    aiSettings.model,
                    aiRuntime.recommendedModel,
                  ]),
                ).map((name) => (
                  <option key={name} value={name}>
                    {name === AI_DEFAULT_MODEL
                      ? `${name} (${t("aiSetupRecommended")})`
                      : name === AI_CPU_MODEL
                        ? `${name} (${t("aiSetupLowSpec")})`
                        : name === aiRuntime.recommendedModel
                          ? `${name} (${t("aiSetupRecommended")})`
                          : name}
                  </option>
                ))}
              </Select>
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
          <div className="grid gap-2 rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] bg-[color-mix(in_srgb,var(--nova-bg)_55%,var(--nova-surface))] p-3">
            <p className="text-sm font-medium">{t("aiPullTitle")}</p>
            <p className="text-xs text-[var(--nova-ink-muted)]">
              {t("aiPullDescription")}
            </p>
            <Input
              value={pullRef}
              disabled={pullBusy}
              placeholder={t("aiPullPlaceholder")}
              onChange={(e) => setPullRef(e.target.value)}
            />
            <div className="flex flex-wrap gap-2">
              <Button
                type="button"
                size="sm"
                disabled={pullBusy || !pullRef.trim()}
                onClick={() => void handlePullModel()}
              >
                {pullBusy ? t("aiPullBusy") : t("aiPullButton")}
              </Button>
              <Button
                type="button"
                size="sm"
                variant="secondary"
                disabled={pullBusy}
                onClick={() => void handlePullModel(AI_DEFAULT_MODEL)}
              >
                {t("aiPullQuickDefault")}
              </Button>
              <Button
                type="button"
                size="sm"
                variant="ghost"
                disabled={pullBusy}
                onClick={() => void handlePullModel(AI_CPU_MODEL)}
              >
                {t("aiPullQuickLowSpec")}
              </Button>
            </div>
            {pullStatus ? (
              <p className="text-xs text-[var(--nova-ink-muted)]">{pullStatus}</p>
            ) : null}
          </div>
          <div className="flex flex-wrap gap-2">
            <Button
              type="button"
              size="sm"
              disabled={aiBusy || pullBusy}
              onClick={() => void handleSaveAi()}
            >
              {aiBusy ? t("working") : t("aiSettingsSave")}
            </Button>
            <Button
              type="button"
              size="sm"
              variant="ghost"
              disabled={aiBusy || pullBusy}
              onClick={() => {
                setError(null);
                setAiStatus(t("aiSettingsRefreshing"));
                void refreshAi()
                  .then(() =>
                    api.aiRuntimeStatus().then((runtime) => {
                      const parts = [
                        runtime.ollamaReachable
                          ? t("aiRuntimeOllamaYes")
                          : t("aiRuntimeOllamaNo"),
                        runtime.nvidiaGpu
                          ? t("aiRuntimeGpuYes")
                          : t("aiRuntimeGpuNo"),
                        runtime.models.length > 0
                          ? t("aiSettingsModelsCount", {
                              count: runtime.models.length,
                            })
                          : t("aiSettingsNoModels"),
                      ];
                      setAiStatus(
                        `${t("aiSettingsRefreshed")} · ${parts.join(" · ")}`,
                      );
                      setAiRuntime(runtime);
                    }),
                  )
                  .catch((err) => {
                    setAiStatus(null);
                    setError((err as AppError).message);
                  });
              }}
            >
              {t("aiSettingsRefresh")}
            </Button>
          </div>
          {aiStatus ? (
            <p className="text-xs text-[var(--nova-ink-muted)]" role="status">
              {aiStatus}
            </p>
          ) : null}
        </section>

        </>
        ) : null}
        {settingsTab === "privacy" ? (
        <>
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
              variant="secondary"
              className="nova-file-btn"
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

        <PgpKeysPanel />
        </>
        ) : null}
        {settingsTab === "mail" ? (
        <>
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

        <OfflineMailboxPanel accounts={accounts} />
        <RulesSpamPanel accounts={accounts} />
        </>
        ) : null}

        {error ? (
          <p className="text-[var(--nova-danger)]" role="alert">
            {error}
          </p>
        ) : null}
      </div>
      <DialogActions>
        <Button onClick={onClose}>{t("done")}</Button>
      </DialogActions>
      </div>
    </Dialog>
  );
}

function SpellDictRow({
  dict,
  activeCode,
  busyCode,
  onUse,
  onInstall,
}: {
  dict: SpellDictionaryDto;
  activeCode: string;
  busyCode: string | null;
  onUse: (code: string) => void;
  onInstall: (code: string) => void;
}) {
  const t = useT();
  return (
    <li className="flex flex-wrap items-center justify-between gap-2 rounded-[var(--nova-radius-sm)] bg-[var(--nova-surface-2)] px-2 py-1.5">
      <span className="min-w-0">
        <span className="block font-medium">{dict.name}</span>
        <span className="text-xs text-[var(--nova-ink-muted)]">
          {dict.installed
            ? `${t("spellcheckInstalled")} · ${
                dict.source === "system"
                  ? t("spellcheckSourceSystem")
                  : t("spellcheckSourceUser")
              }`
            : t("spellcheckMissing")}
        </span>
      </span>
      {dict.installed ? (
        <Button
          type="button"
          size="sm"
          variant="secondary"
          disabled={activeCode === dict.code}
          onClick={() => onUse(dict.code)}
        >
          {activeCode === dict.code
            ? t("spellcheckInUse")
            : t("spellcheckUse")}
        </Button>
      ) : (
        <Button
          type="button"
          size="sm"
          className="nova-file-btn"
          variant="secondary"
          disabled={busyCode === dict.code}
          onClick={() => onInstall(dict.code)}
        >
          {busyCode === dict.code
            ? t("spellcheckInstalling")
            : t("spellcheckInstall")}
        </Button>
      )}
    </li>
  );
}
