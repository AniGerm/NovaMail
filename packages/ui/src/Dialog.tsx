import type { PropsWithChildren, ReactNode } from "react";
import { cn } from "./utils";
import { Overlay } from "./Overlay";

export function Dialog({
  open,
  title,
  description,
  children,
  onClose,
  className,
  headerActions,
  dense = false,
}: PropsWithChildren<{
  open: boolean;
  title: string;
  description?: string;
  onClose: () => void;
  className?: string;
  headerActions?: ReactNode;
  /** Single-line title and tighter spacing. */
  dense?: boolean;
}>) {
  if (!open) return null;
  return (
    <Overlay
      role="dialog"
      aria-modal="true"
      aria-labelledby="nova-dialog-title"
    >
      <div
        className={cn(
          "relative flex max-h-full w-full max-w-lg flex-col overflow-hidden rounded-[var(--nova-radius-lg)] border border-[var(--nova-border)] bg-[var(--nova-surface)] p-6 shadow-[var(--nova-shadow)]",
          className,
        )}
      >
        <div
          className={cn(
            "mb-4 flex shrink-0 items-start justify-between gap-3",
            dense && "mb-1 items-center",
          )}
        >
          <div className="min-w-0">
            <h2
              id="nova-dialog-title"
              className={cn(
                "font-[family-name:var(--nova-font-display)] text-xl",
                dense && "truncate text-sm font-medium leading-tight",
              )}
            >
              {title}
            </h2>
            {description ? (
              <p
                className={cn(
                  "mt-1 text-sm text-[var(--nova-ink-muted)]",
                  dense && "truncate text-xs",
                )}
              >
                {description}
              </p>
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
    </Overlay>
  );
}

export function DialogActions({ children }: { children: ReactNode }) {
  return (
    <div className="mt-4 flex shrink-0 justify-end gap-2 border-t border-[var(--nova-border)] pt-4">
      {children}
    </div>
  );
}
