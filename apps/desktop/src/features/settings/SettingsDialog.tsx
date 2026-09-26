import { useEffect, useState } from "react";
import { Button, Dialog, DialogActions, Input } from "@novamail/ui";

import { api } from "@/shared/api/client";
import type { AccountDto, AppError, LabelDto, RuleDto, SignatureDto } from "@/shared/api/types";
import { useT } from "@/shared/i18n/useT";
import type { Locale } from "@/shared/i18n";
import type { ColorSchemeId } from "@/shared/theme/schemes";
import { useUiStore } from "@/shared/store/uiStore";

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
  const [error, setError] = useState<string | null>(null);

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

  useEffect(() => {
    if (!open) return;
    setSigName(t("defaultSignatureName"));
    refreshExtras().catch((err) => setError((err as AppError).message));
  }, [open, t]);

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

        <div className="grid grid-cols-4 gap-2 px-0.5 py-1" aria-hidden>
          {(
            [
              ["navy", "#1e3a5f", "#d7e4f4"],
              ["forest", "#0b6e4f", "#d8f3e7"],
              ["slate", "#334155", "#e2e8f0"],
              ["midnight", "#312e81", "#e0e7ff"],
            ] as const
          ).map(([id, accent, soft]) => (
            <button
              key={id}
              type="button"
              title={id}
              onClick={() => setColorScheme(id)}
              className="h-7 w-full rounded-[6px] border border-[var(--nova-border)]"
              style={{
                background: `linear-gradient(135deg, ${soft} 55%, ${accent})`,
                boxShadow:
                  colorScheme === id
                    ? "0 0 0 2px var(--nova-surface), 0 0 0 4px var(--nova-accent)"
                    : undefined,
              }}
            />
          ))}
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
