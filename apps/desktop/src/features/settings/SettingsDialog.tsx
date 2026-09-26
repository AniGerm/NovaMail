import { useEffect, useState } from "react";
import { Button, Dialog, DialogActions, Input } from "@novamail/ui";

import { api } from "@/shared/api/client";
import type { AccountDto, AppError, LabelDto, RuleDto, SignatureDto } from "@/shared/api/types";
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
  const theme = useUiStore((s) => s.theme);
  const setTheme = useUiStore((s) => s.setTheme);
  const highContrast = useUiStore((s) => s.highContrast);
  const setHighContrast = useUiStore((s) => s.setHighContrast);
  const density = useUiStore((s) => s.density);
  const setDensity = useUiStore((s) => s.setDensity);

  const [signatures, setSignatures] = useState<SignatureDto[]>([]);
  const [labels, setLabels] = useState<LabelDto[]>([]);
  const [rules, setRules] = useState<RuleDto[]>([]);
  const [sigName, setSigName] = useState("Default");
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
    refreshExtras().catch((err) => setError((err as AppError).message));
  }, [open]);

  return (
    <Dialog
      open={open}
      onClose={onClose}
      title="Settings"
      description="Appearance, signatures, labels, and mail rules."
    >
      <div className="grid max-h-[70vh] gap-5 overflow-y-auto text-sm">
        <label className="grid gap-1">
          <span>Theme</span>
          <select
            className="h-11 rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] bg-[var(--nova-surface)] px-3"
            value={theme}
            onChange={(e) => setTheme(e.target.value as "light" | "dark" | "system")}
          >
            <option value="system">System</option>
            <option value="light">Light</option>
            <option value="dark">Dark</option>
          </select>
        </label>
        <label className="grid gap-1">
          <span>Density</span>
          <select
            className="h-11 rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] bg-[var(--nova-surface)] px-3"
            value={density}
            onChange={(e) =>
              setDensity(e.target.value as "comfortable" | "compact")
            }
          >
            <option value="comfortable">Comfortable</option>
            <option value="compact">Compact</option>
          </select>
        </label>
        <label className="flex items-center gap-2">
          <input
            type="checkbox"
            checked={highContrast}
            onChange={(e) => setHighContrast(e.target.checked)}
          />
          High contrast mode
        </label>

        <section className="grid gap-2">
          <h3 className="font-medium">Signatures</h3>
          <Input value={sigName} onChange={(e) => setSigName(e.target.value)} />
          <textarea
            className="min-h-[80px] rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] bg-[var(--nova-surface)] px-3 py-2"
            value={sigBody}
            onChange={(e) => setSigBody(e.target.value)}
            placeholder="Signature text"
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
            Save signature
          </Button>
          <ul className="space-y-1 text-[var(--nova-ink-muted)]">
            {signatures.map((sig) => (
              <li key={sig.id}>
                {sig.name}
                {sig.isDefault ? " (default)" : ""}
              </li>
            ))}
          </ul>
        </section>

        <section className="grid gap-2">
          <h3 className="font-medium">Labels</h3>
          <Input
            placeholder="Label name"
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
                color: "#0B6E4F",
              });
              setLabelName("");
              await refreshExtras();
            }}
          >
            Add label
          </Button>
          <ul className="space-y-1 text-[var(--nova-ink-muted)]">
            {labels.map((label) => (
              <li key={label.id}>{label.name}</li>
            ))}
          </ul>
        </section>

        <section className="grid gap-2">
          <h3 className="font-medium">Rules</h3>
          <Input
            placeholder="Rule name"
            value={ruleName}
            onChange={(e) => setRuleName(e.target.value)}
          />
          <Input
            placeholder="Subject contains…"
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
            Add rule
          </Button>
          <ul className="space-y-1 text-[var(--nova-ink-muted)]">
            {rules.map((rule) => (
              <li key={rule.id}>
                {rule.name} {rule.enabled ? "" : "(disabled)"}
              </li>
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
        <Button onClick={onClose}>Done</Button>
      </DialogActions>
    </Dialog>
  );
}
