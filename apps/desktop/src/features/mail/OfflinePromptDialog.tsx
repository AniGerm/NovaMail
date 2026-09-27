import { Button, Dialog, DialogActions } from "@novamail/ui";

import type { OfflinePromptEvent } from "@/shared/api/types";
import { useT } from "@/shared/i18n/useT";

function formatBytes(bytes: number): string {
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  if (bytes < 1024 * 1024 * 1024)
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

export function OfflinePromptDialog({
  prompt,
  onEnable,
  onDismiss,
}: {
  prompt: OfflinePromptEvent | null;
  onEnable: () => void;
  onDismiss: () => void;
}) {
  const t = useT();
  if (!prompt) return null;

  return (
    <Dialog
      open
      onClose={onDismiss}
      title={t("offlinePromptTitle")}
      description={t("offlinePromptBody", {
        email: prompt.accountEmail,
        percent: Math.round(prompt.percent),
        used: formatBytes(prompt.usedBytes),
        limit: prompt.limitBytes
          ? formatBytes(prompt.limitBytes)
          : t("offlineQuotaUnknown"),
      })}
    >
      <p className="text-sm text-[var(--nova-ink-muted)]">
        {t("offlinePromptHint")}
      </p>
      <DialogActions>
        <Button type="button" variant="secondary" onClick={onDismiss}>
          {t("offlinePromptLater")}
        </Button>
        <Button type="button" onClick={onEnable}>
          {t("offlinePromptEnable")}
        </Button>
      </DialogActions>
    </Dialog>
  );
}
