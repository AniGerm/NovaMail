import { useEffect, useState } from "react";
import { Button, Input, Select } from "@novamail/ui";

import { api } from "@/shared/api/client";
import type {
  AccountDto,
  AppError,
  FolderPoliciesDto,
  RuleDto,
  SpamSettingsDto,
} from "@/shared/api/types";
import { useT } from "@/shared/i18n/useT";

type PredType =
  | "always"
  | "fromContains"
  | "toContains"
  | "subjectContains"
  | "bodyContains";
type ActionType =
  | "markRead"
  | "markUnread"
  | "star"
  | "unstar"
  | "addLabel"
  | "moveToMailbox"
  | "moveToSpam"
  | "delete";

interface RuleForm {
  id?: string | null;
  name: string;
  enabled: boolean;
  accountId: string;
  predType: PredType;
  predValue: string;
  actionType: ActionType;
  actionValue: string;
}

const emptyForm = (): RuleForm => ({
  id: null,
  name: "",
  enabled: true,
  accountId: "",
  predType: "subjectContains",
  predValue: "",
  actionType: "markRead",
  actionValue: "",
});

function parseRule(rule: RuleDto): RuleForm {
  let predType: PredType = "subjectContains";
  let predValue = "";
  try {
    const pred = JSON.parse(rule.predicateJson) as {
      type?: string;
      value?: string;
    };
    if (pred.type === "always") predType = "always";
    else if (pred.type === "fromContains") predType = "fromContains";
    else if (pred.type === "toContains") predType = "toContains";
    else if (pred.type === "bodyContains") predType = "bodyContains";
    else predType = "subjectContains";
    predValue = pred.value ?? "";
  } catch {
    /* keep defaults */
  }
  let actionType: ActionType = "markRead";
  let actionValue = "";
  try {
    const acts = JSON.parse(rule.actionJson) as Array<{
      type?: string;
      label?: string;
      mailbox?: string;
    }>;
    const first = acts[0] ?? {};
    switch (first.type) {
      case "markUnread":
        actionType = "markUnread";
        break;
      case "star":
        actionType = "star";
        break;
      case "unstar":
        actionType = "unstar";
        break;
      case "addLabel":
        actionType = "addLabel";
        actionValue = first.label ?? "";
        break;
      case "moveToMailbox":
        actionType = "moveToMailbox";
        actionValue = first.mailbox ?? "";
        break;
      case "moveToSpam":
        actionType = "moveToSpam";
        break;
      case "delete":
        actionType = "delete";
        break;
      default:
        actionType = "markRead";
    }
  } catch {
    /* keep defaults */
  }
  return {
    id: rule.id,
    name: rule.name,
    enabled: rule.enabled,
    accountId: rule.accountId ?? "",
    predType,
    predValue,
    actionType,
    actionValue,
  };
}

function buildPredicateJson(form: RuleForm): string {
  if (form.predType === "always") {
    return JSON.stringify({ type: "always" });
  }
  return JSON.stringify({ type: form.predType, value: form.predValue.trim() });
}

function buildActionJson(form: RuleForm): string {
  switch (form.actionType) {
    case "addLabel":
      return JSON.stringify([{ type: "addLabel", label: form.actionValue.trim() }]);
    case "moveToMailbox":
      return JSON.stringify([
        { type: "moveToMailbox", mailbox: form.actionValue.trim() },
      ]);
    case "moveToSpam":
      return JSON.stringify([{ type: "moveToSpam" }]);
    case "delete":
      return JSON.stringify([{ type: "delete" }]);
    case "markUnread":
      return JSON.stringify([{ type: "markUnread" }]);
    case "star":
      return JSON.stringify([{ type: "star" }]);
    case "unstar":
      return JSON.stringify([{ type: "unstar" }]);
    default:
      return JSON.stringify([{ type: "markRead" }]);
  }
}

export function RulesSpamPanel({ accounts }: { accounts: AccountDto[] }) {
  const t = useT();
  const [rules, setRules] = useState<RuleDto[]>([]);
  const [form, setForm] = useState<RuleForm>(emptyForm);
  const [spam, setSpam] = useState<SpamSettingsDto | null>(null);
  const [policies, setPolicies] = useState<FolderPoliciesDto | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function refresh() {
    const [rls, spamSettings, folderPolicies] = await Promise.all([
      api.rulesList(),
      api.spamGetSettings(),
      api.folderPoliciesGet(),
    ]);
    setRules(rls);
    setSpam(spamSettings);
    setPolicies(folderPolicies);
  }

  useEffect(() => {
    void refresh().catch((err) => setError((err as AppError).message));
  }, []);

  const needsPredValue = form.predType !== "always";
  const needsActionValue =
    form.actionType === "addLabel" || form.actionType === "moveToMailbox";

  async function saveRule() {
    if (!form.name.trim()) return;
    if (needsPredValue && !form.predValue.trim()) return;
    if (needsActionValue && !form.actionValue.trim()) return;
    setBusy(true);
    setError(null);
    try {
      await api.rulesUpsert({
        id: form.id ?? null,
        accountId: form.accountId || null,
        name: form.name.trim(),
        enabled: form.enabled,
        predicateJson: buildPredicateJson(form),
        actionJson: buildActionJson(form),
      });
      setForm(emptyForm());
      setStatus(t("ruleSaved"));
      await refresh();
    } catch (err) {
      setError((err as AppError).message);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="grid gap-5">
      <section className="grid gap-2 rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] p-3">
        <h3 className="font-medium">{t("rules")}</h3>
        <p className="text-xs text-[var(--nova-ink-muted)]">{t("rulesDescription")}</p>

        <div className="grid gap-2 sm:grid-cols-2">
          <label className="grid gap-1 sm:col-span-2">
            <span>{t("ruleName")}</span>
            <Input
              value={form.name}
              onChange={(e) => setForm({ ...form, name: e.target.value })}
              placeholder={t("ruleName")}
            />
          </label>
          <label className="grid gap-1">
            <span>{t("ruleAccount")}</span>
            <Select
              className="h-11"
              value={form.accountId}
              onChange={(e) => setForm({ ...form, accountId: e.target.value })}
            >
              <option value="">{t("ruleAllAccounts")}</option>
              {accounts.map((a) => (
                <option key={a.id} value={a.id}>
                  {a.email}
                </option>
              ))}
            </Select>
          </label>
          <label className="flex items-center gap-2 pt-6">
            <input
              type="checkbox"
              checked={form.enabled}
              onChange={(e) => setForm({ ...form, enabled: e.target.checked })}
            />
            <span>{t("ruleEnabled")}</span>
          </label>
          <label className="grid gap-1">
            <span>{t("ruleWhen")}</span>
            <Select
              className="h-11"
              value={form.predType}
              onChange={(e) =>
                setForm({ ...form, predType: e.target.value as PredType })
              }
            >
              <option value="subjectContains">{t("rulePredSubject")}</option>
              <option value="fromContains">{t("rulePredFrom")}</option>
              <option value="toContains">{t("rulePredTo")}</option>
              <option value="bodyContains">{t("rulePredBody")}</option>
              <option value="always">{t("rulePredAlways")}</option>
            </Select>
          </label>
          {needsPredValue ? (
            <label className="grid gap-1">
              <span>{t("ruleContains")}</span>
              <Input
                value={form.predValue}
                onChange={(e) => setForm({ ...form, predValue: e.target.value })}
                placeholder={t("ruleContains")}
              />
            </label>
          ) : null}
          <label className="grid gap-1">
            <span>{t("ruleThen")}</span>
            <Select
              className="h-11"
              value={form.actionType}
              onChange={(e) =>
                setForm({ ...form, actionType: e.target.value as ActionType })
              }
            >
              <option value="markRead">{t("ruleActMarkRead")}</option>
              <option value="markUnread">{t("ruleActMarkUnread")}</option>
              <option value="star">{t("ruleActStar")}</option>
              <option value="unstar">{t("ruleActUnstar")}</option>
              <option value="addLabel">{t("ruleActAddLabel")}</option>
              <option value="moveToMailbox">{t("ruleActMove")}</option>
              <option value="moveToSpam">{t("ruleActSpam")}</option>
              <option value="delete">{t("ruleActDelete")}</option>
            </Select>
          </label>
          {needsActionValue ? (
            <label className="grid gap-1">
              <span>
                {form.actionType === "addLabel"
                  ? t("labelName")
                  : t("ruleMailboxHint")}
              </span>
              <Input
                value={form.actionValue}
                onChange={(e) =>
                  setForm({ ...form, actionValue: e.target.value })
                }
                placeholder={
                  form.actionType === "addLabel"
                    ? t("labelName")
                    : "Spam / Archive / INBOX"
                }
              />
            </label>
          ) : null}
        </div>
        <div className="flex flex-wrap gap-2">
          <Button type="button" size="sm" disabled={busy} onClick={() => void saveRule()}>
            {form.id ? t("ruleUpdate") : t("addRule")}
          </Button>
          {form.id ? (
            <Button
              type="button"
              size="sm"
              variant="secondary"
              onClick={() => setForm(emptyForm())}
            >
              {t("ruleCancelEdit")}
            </Button>
          ) : null}
        </div>

        <ul className="grid gap-1.5">
          {rules.map((rule) => (
            <li
              key={rule.id}
              className="flex flex-wrap items-center justify-between gap-2 rounded-[var(--nova-radius-sm)] bg-[var(--nova-surface-2)] px-2 py-1.5"
            >
              <span className="min-w-0">
                <span className="block font-medium">
                  {rule.name}
                  {!rule.enabled ? (
                    <span className="ml-2 text-xs text-[var(--nova-ink-muted)]">
                      ({t("ruleDisabled")})
                    </span>
                  ) : null}
                </span>
                <span className="text-xs text-[var(--nova-ink-muted)]">
                  {rule.predicateJson} → {rule.actionJson}
                </span>
              </span>
              <span className="flex gap-1">
                <Button
                  type="button"
                  size="sm"
                  variant="secondary"
                  onClick={() => setForm(parseRule(rule))}
                >
                  {t("edit")}
                </Button>
                <Button
                  type="button"
                  size="sm"
                  variant="secondary"
                  onClick={() => {
                    void api
                      .rulesDelete(rule.id)
                      .then(() => refresh())
                      .catch((err) => setError((err as AppError).message));
                  }}
                >
                  {t("delete")}
                </Button>
              </span>
            </li>
          ))}
        </ul>
      </section>

      {spam ? (
        <section className="grid gap-2 rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] p-3">
          <h3 className="font-medium">{t("spamTitle")}</h3>
          <p className="text-xs text-[var(--nova-ink-muted)]">{t("spamDescription")}</p>
          <label className="flex items-center gap-2">
            <input
              type="checkbox"
              checked={spam.enabled}
              onChange={(e) => setSpam({ ...spam, enabled: e.target.checked })}
            />
            <span>{t("spamEnabled")}</span>
          </label>
          <label className="flex items-center gap-2">
            <input
              type="checkbox"
              checked={spam.autoMove}
              onChange={(e) => setSpam({ ...spam, autoMove: e.target.checked })}
            />
            <span>{t("spamAutoMove")}</span>
          </label>
          <label className="flex items-center gap-2">
            <input
              type="checkbox"
              checked={spam.strictHeuristics}
              onChange={(e) =>
                setSpam({ ...spam, strictHeuristics: e.target.checked })
              }
            />
            <span>{t("spamStrictHeuristics")}</span>
          </label>
          <label className="grid gap-1">
            <span>
              {t("spamThreshold")}: {Math.round(spam.threshold * 100)}%
            </span>
            <input
              type="range"
              min={40}
              max={99}
              value={Math.round(spam.threshold * 100)}
              onChange={(e) =>
                setSpam({
                  ...spam,
                  threshold: Number(e.target.value) / 100,
                })
              }
            />
          </label>
          <p className="text-xs text-[var(--nova-ink-muted)]">
            {t("spamTrained", {
              spam: spam.trainedSpam,
              ham: spam.trainedHam,
            })}
          </p>
          <Button
            type="button"
            size="sm"
            onClick={() => {
              void api
                .spamSetSettings(spam)
                .then((next) => {
                  setSpam(next);
                  setStatus(t("spamSaved"));
                })
                .catch((err) => setError((err as AppError).message));
            }}
          >
            {t("spamSave")}
          </Button>
        </section>
      ) : null}

      {policies ? (
        <section className="grid gap-2 rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] p-3">
          <h3 className="font-medium">{t("folderPoliciesTitle")}</h3>
          <p className="text-xs text-[var(--nova-ink-muted)]">
            {t("folderPoliciesDescription")}
          </p>
          {policies.policies.map((policy, idx) => (
            <div
              key={policy.role}
              className="grid gap-2 rounded-[var(--nova-radius-sm)] bg-[var(--nova-surface-2)] p-2 sm:grid-cols-3"
            >
              <div className="font-medium">
                {policy.role === "junk"
                  ? t("spam")
                  : policy.role === "trash"
                    ? t("trash")
                    : policy.role}
              </div>
              <Select
                className="h-11"
                value={policy.mode}
                onChange={(e) => {
                  const next = { ...policies, policies: [...policies.policies] };
                  next.policies[idx] = {
                    ...policy,
                    mode: e.target.value as "keep" | "deleteAfterDays",
                  };
                  setPolicies(next);
                }}
              >
                <option value="keep">{t("retentionKeep")}</option>
                <option value="deleteAfterDays">{t("retentionDeleteAfter")}</option>
              </Select>
              {policy.mode === "deleteAfterDays" ? (
                <label className="grid gap-1">
                  <span className="text-xs">{t("retentionDays")}</span>
                  <Input
                    type="number"
                    min={1}
                    max={3650}
                    value={policy.days}
                    onChange={(e) => {
                      const next = {
                        ...policies,
                        policies: [...policies.policies],
                      };
                      next.policies[idx] = {
                        ...policy,
                        days: Number(e.target.value) || 1,
                      };
                      setPolicies(next);
                    }}
                  />
                </label>
              ) : (
                <span className="text-xs text-[var(--nova-ink-muted)] self-center">
                  {t("retentionKeepHint")}
                </span>
              )}
            </div>
          ))}
          <div className="flex flex-wrap gap-2">
            <Button
              type="button"
              size="sm"
              onClick={() => {
                void api
                  .folderPoliciesSet(policies)
                  .then((next) => {
                    setPolicies(next);
                    setStatus(t("folderPoliciesSaved"));
                  })
                  .catch((err) => setError((err as AppError).message));
              }}
            >
              {t("folderPoliciesSave")}
            </Button>
            <Button
              type="button"
              size="sm"
              variant="secondary"
              onClick={() => {
                void api
                  .folderPoliciesApply()
                  .then((n) => setStatus(t("folderPoliciesApplied", { count: n })))
                  .catch((err) => setError((err as AppError).message));
              }}
            >
              {t("folderPoliciesApplyNow")}
            </Button>
          </div>
        </section>
      ) : null}

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
