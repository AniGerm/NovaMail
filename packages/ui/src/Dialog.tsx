import type { PropsWithChildren, ReactNode } from "react";
import { cn } from "./utils";

export function Dialog({
  open,
  title,
  description,
  children,
  onClose,
  className,
  headerActions,
}: PropsWithChildren<{
  open: boolean;
  title: string;
  description?: string;
  onClose: () => void;
  className?: string;
  headerActions?: ReactNode;
}>) {
  if (!open) return null;
  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-[rgba(14,17,20,0.45)] p-4 backdrop-blur-sm"
      role="dialog"
      aria-modal="true"
      aria-labelledby="nova-dialog-title"
    >
      <div
        className={cn(
          "flex max-h-[90vh] w-full max-w-lg flex-col overflow-hidden rounded-[var(--nova-radius-lg)] border border-[var(--nova-border)] bg-[var(--nova-surface)] p-6 shadow-[var(--nova-shadow)]",
          className,
        )}
      >
        <div className="mb-4 flex shrink-0 items-start justify-between gap-3">
          <div className="min-w-0">
            <h2
              id="nova-dialog-title"
              className="font-[family-name:var(--nova-font-display)] text-xl"
            >
              {title}
            </h2>
            {description ? (
              <p className="mt-1 text-sm text-[var(--nova-ink-muted)]">{description}</p>
            ) : null}
          </div>
          <div className="flex shrink-0 items-center gap-1">
            {headerActions}
            <button
              type="button"
              className="px-1 text-sm text-[var(--nova-ink-muted)]"
              onClick={onClose}
            >
              ×
            </button>
          </div>
        </div>
        <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
          {children}
        </div>
      </div>
    </div>
  );
}

export function DialogActions({ children }: { children: ReactNode }) {
  return (
    <div className="mt-4 flex shrink-0 justify-end gap-2 border-t border-[var(--nova-border)] pt-4">
      {children}
    </div>
  );
}
