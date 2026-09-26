import type { PropsWithChildren } from "react";
import { cn } from "./utils";

export function Badge({
  children,
  className,
}: PropsWithChildren<{ className?: string }>) {
  return (
    <span
      className={cn(
        "inline-flex items-center rounded-[var(--nova-radius-sm)] bg-[var(--nova-accent-soft)] px-2 py-0.5 text-xs font-medium text-[var(--nova-accent)]",
        className,
      )}
    >
      {children}
    </span>
  );
}
