import { useEffect, useState } from "react";
import { open as openUrl } from "@tauri-apps/plugin-shell";
import { Button } from "@novamail/ui";

import { api, isDesktopShell } from "@/shared/api/client";
import type { AiRuntimeStatus, AiSettings, AppError } from "@/shared/api/types";
import { useT } from "@/shared/i18n/useT";

const DEFAULT_MODEL = "qwen3:4b-instruct";
const CPU_MODEL = "qwen2.5:1.5b";

interface AiSetupDialogProps {
  open: boolean;
  onCompleted: (settings: AiSettings) => void;
}

export function AiSetupDialog({ open, onCompleted }: AiSetupDialogProps) {
  const t = useT();
  const [step, setStep] = useState<"ask" | "install" | "model">("ask");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [runtime, setRuntime] = useState<AiRuntimeStatus | null>(null);
  const [model, setModel] = useState(DEFAULT_MODEL);
  const [baseUrl, setBaseUrl] = useState("http://127.0.0.1:11434");
  const [installStatus, setInstallStatus] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    setStep("ask");
    setError(null);
    setBusy(false);
    setRuntime(null);
    setInstallStatus(null);
    if (!isDesktopShell()) return;
    void api
      .aiGetSettings()
      .then((settings) => {
        setModel(settings.model || DEFAULT_MODEL);
        setBaseUrl(settings.baseUrl || "http://127.0.0.1:11434");
      })
      .catch(() => undefined);
    // Probe early so "Yes" can skip straight to install if needed.
    void api
      .aiRuntimeStatus()
      .then((status) => {
        setRuntime(status);
        setModel(status.recommendedModel || DEFAULT_MODEL);
      })
      .catch(() => undefined);
  }, [open]);

  async function loadRuntime(): Promise<AiRuntimeStatus> {
    setBusy(true);
    setError(null);
    try {
      const status = await api.aiRuntimeStatus();
      setRuntime(status);
      setModel(status.recommendedModel || DEFAULT_MODEL);
      return status;
    } catch (err) {
      setError((err as AppError).message);
      const fallback: AiRuntimeStatus = {
        ollamaReachable: false,
        ollamaInstalled: false,
        ollamaBinary: null,
        canInstallUser: false,
        canInstallSystem: false,
        nvidiaGpu: false,
        models: [],
        recommendedModel: CPU_MODEL,
        allowModelPick: false,
      };
      setRuntime(fallback);
      setModel(CPU_MODEL);
      return fallback;
    } finally {
      setBusy(false);
    }
  }

  async function save(enabled: boolean, nextModel: string) {
    setBusy(true);
    setError(null);
    try {
      const settings = await api.aiSetSettings({
        enabled,
        model: nextModel.trim() || DEFAULT_MODEL,
        baseUrl: baseUrl.trim() || "http://127.0.0.1:11434",
        onboardingCompleted: true,
      });
      onCompleted(settings);
    } catch (err) {
      setError((err as AppError).message);
    } finally {
      setBusy(false);
    }
  }

  async function chooseNo() {
    await save(false, DEFAULT_MODEL);
  }

  async function continueAfterOllama(status: AiRuntimeStatus) {
    if (!status.allowModelPick) {
      await save(true, status.recommendedModel || CPU_MODEL);
      return;
    }
    setStep("model");
  }

  async function chooseYes() {
    const status = runtime ?? (await loadRuntime());
    if (!status.ollamaReachable) {
      setStep("install");
      return;
    }
    await continueAfterOllama(status);
  }

  async function handleInstall(mode: "user" | "system") {
    setBusy(true);
    setError(null);
    setInstallStatus(t("aiInstallBusy"));
    let unlisten: (() => void) | undefined;
    try {
      unlisten = await api.onAiInstallProgress((event) => {
        setInstallStatus(event.status);
      });
      await api.aiInstallOllama(mode);
      const status = await loadRuntime();
      setInstallStatus(t("aiInstallReady"));
      if (status.ollamaReachable) {
        await continueAfterOllama(status);
      }
    } catch (err) {
      setError((err as AppError).message);
      setInstallStatus(null);
    } finally {
      unlisten?.();
      setBusy(false);
    }
  }

  async function handleStart() {
    setBusy(true);
    setError(null);
    try {
      await api.aiStartOllama();
      const status = await loadRuntime();
      setInstallStatus(t("aiInstallReady"));
      if (status.ollamaReachable) {
        await continueAfterOllama(status);
      }
    } catch (err) {
      setError((err as AppError).message);
    } finally {
      setBusy(false);
    }
  }

  if (!open) return null;

  const modelOptions = Array.from(
    new Set([
      DEFAULT_MODEL,
      CPU_MODEL,
      ...(runtime?.models ?? []),
      runtime?.recommendedModel || DEFAULT_MODEL,
    ]),
  );

  return (
    <div
      className="fixed inset-0 z-[60] flex items-center justify-center bg-[rgba(14,17,20,0.45)] p-4 backdrop-blur-sm"
      role="dialog"
      aria-modal="true"
      aria-labelledby="ai-setup-title"
    >
      <div className="nova-fade-in w-full max-w-lg rounded-[var(--nova-radius-lg)] border border-[var(--nova-border)] bg-[var(--nova-surface)] p-6 shadow-[var(--nova-shadow)]">
        <h2
          id="ai-setup-title"
          className="font-[family-name:var(--nova-font-display)] text-2xl"
        >
          {step === "install" ? t("aiInstallTitle") : t("aiSetupTitle")}
        </h2>
        <p className="mt-1 text-sm text-[var(--nova-ink-muted)]">
          {step === "install"
            ? t("aiInstallDescription")
            : t("aiSetupDescription")}
        </p>

        {step === "ask" ? (
          <div className="mt-5 grid gap-3">
            {runtime ? (
              <p className="text-xs text-[var(--nova-ink-muted)]">
                {runtime.ollamaReachable
                  ? t("aiRuntimeOllamaYes")
                  : runtime.ollamaInstalled
                    ? t("aiInstallFound")
                    : t("aiInstallMissing")}
                {" · "}
                {runtime.nvidiaGpu ? t("aiRuntimeGpuYes") : t("aiRuntimeGpuNo")}
              </p>
            ) : null}
            {error ? (
              <p className="text-sm text-[var(--nova-danger)]" role="alert">
                {error}
              </p>
            ) : null}
            <div className="flex flex-wrap justify-end gap-2">
              <Button
                type="button"
                variant="secondary"
                disabled={busy}
                onClick={() => void chooseNo()}
              >
                {t("aiSetupNo")}
              </Button>
              <Button
                type="button"
                disabled={busy}
                onClick={() => void chooseYes()}
              >
                {busy ? t("working") : t("aiSetupYes")}
              </Button>
            </div>
          </div>
        ) : null}

        {step === "install" ? (
          <div className="mt-5 grid gap-3">
            <p className="text-xs text-[var(--nova-ink-muted)]">
              {runtime?.ollamaInstalled
                ? t("aiInstallFound")
                : t("aiInstallMissing")}
            </p>
            {installStatus ? (
              <p className="text-xs text-[var(--nova-ink-muted)]">{installStatus}</p>
            ) : null}
            {error ? (
              <p className="text-sm text-[var(--nova-danger)]" role="alert">
                {error}
              </p>
            ) : null}
            <div className="flex flex-wrap gap-2">
              {runtime?.ollamaInstalled && !runtime.ollamaReachable ? (
                <Button
                  type="button"
                  disabled={busy}
                  onClick={() => void handleStart()}
                >
                  {busy ? t("working") : t("aiInstallStart")}
                </Button>
              ) : null}
              {runtime?.canInstallUser ? (
                <Button
                  type="button"
                  disabled={busy}
                  onClick={() => void handleInstall("user")}
                >
                  {busy ? t("aiInstallBusy") : t("aiInstallUser")}
                </Button>
              ) : null}
              {runtime?.canInstallSystem ? (
                <Button
                  type="button"
                  variant="secondary"
                  disabled={busy}
                  onClick={() => void handleInstall("system")}
                >
                  {t("aiInstallSystem")}
                </Button>
              ) : null}
              <Button
                type="button"
                variant="ghost"
                disabled={busy}
                onClick={() => void openUrl("https://ollama.com/download")}
              >
                {t("aiInstallOpenSite")}
              </Button>
            </div>
            <div className="flex flex-wrap justify-end gap-2">
              <Button
                type="button"
                variant="secondary"
                disabled={busy}
                onClick={() => setStep("ask")}
              >
                {t("cancel")}
              </Button>
              <Button
                type="button"
                variant="ghost"
                disabled={busy}
                onClick={() => void save(true, runtime?.recommendedModel || CPU_MODEL)}
              >
                {t("aiSetupContinue")}
              </Button>
            </div>
          </div>
        ) : null}

        {step === "model" ? (
          <div className="mt-5 grid gap-3">
            <p className="text-sm text-[var(--nova-ink-muted)]">
              {t("aiSetupGpuHint")}
            </p>
            {!runtime?.ollamaReachable ? (
              <p className="text-xs text-[var(--nova-ink-muted)]">
                {t("aiSetupOllamaMissing")}
              </p>
            ) : null}
            <label className="grid gap-1 text-sm">
              <span>{t("aiSetupModelLabel")}</span>
              <select
                className="h-11 rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] bg-[var(--nova-surface)] px-3"
                value={model}
                onChange={(e) => setModel(e.target.value)}
              >
                {modelOptions.map((name) => (
                  <option key={name} value={name}>
                    {name === DEFAULT_MODEL
                      ? `${name} (${t("aiSetupRecommended")})`
                      : name === CPU_MODEL
                        ? `${name} (${t("aiSetupLowSpec")})`
                        : name === runtime?.recommendedModel
                          ? `${name} (${t("aiSetupRecommended")})`
                          : name}
                  </option>
                ))}
              </select>
            </label>
            <label className="grid gap-1 text-sm">
              <span>{t("aiSettingsBaseUrl")}</span>
              <input
                className="h-11 rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] bg-[var(--nova-surface)] px-3"
                value={baseUrl}
                onChange={(e) => setBaseUrl(e.target.value)}
              />
            </label>
            {error ? (
              <p className="text-sm text-[var(--nova-danger)]" role="alert">
                {error}
              </p>
            ) : null}
            <div className="flex flex-wrap justify-end gap-2">
              <Button
                type="button"
                variant="secondary"
                disabled={busy}
                onClick={() => setStep("ask")}
              >
                {t("cancel")}
              </Button>
              <Button
                type="button"
                disabled={busy || !model.trim()}
                onClick={() => void save(true, model)}
              >
                {busy ? t("working") : t("aiSetupContinue")}
              </Button>
            </div>
          </div>
        ) : null}
      </div>
    </div>
  );
}
