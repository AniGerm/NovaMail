import type { PropsWithChildren, ReactNode } from "react";
import { cn } from "./utils";

export function Dialog({
  open,
  title,
  description,
  children,
  onClose,
  className,
}: PropsWithChildren<{
  open: boolean;
  title: string;
  description?: string;
  onClose: () => void;
  className?: string;
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
          "w-full max-w-lg rounded-[var(--nova-radius-lg)] border border-[var(--nova-border)] bg-[var(--nova-surface)] p-6 shadow-[var(--nova-shadow)]",
          className,
        )}
      >
        <div className="mb-4 flex items-start justify-between gap-3">
          <div>
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
          <button
            type="button"
            className="text-sm text-[var(--nova-ink-muted)]"
            onClick={onClose}
          >
            ×
          </button>
        </div>
        {children}
      </div>
    </div>
  );
}

export function DialogActions({ children }: { children: ReactNode }) {
  return <div className="mt-6 flex justify-end gap-2">{children}</div>;
}
