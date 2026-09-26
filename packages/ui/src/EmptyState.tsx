import type { PropsWithChildren, ReactNode } from "react";
import { cn } from "./utils";

export function EmptyState({
  title,
  description,
  action,
  className,
}: PropsWithChildren<{
  title: string;
  description: string;
  action?: ReactNode;
  className?: string;
}>) {
  return (
    <div
      className={cn(
        "flex h-full flex-col items-center justify-center gap-3 px-8 text-center",
        className,
      )}
    >
      <h2 className="font-[family-name:var(--nova-font-display)] text-2xl text-[var(--nova-ink)]">
        {title}
      </h2>
      <p className="max-w-md text-[var(--nova-ink-muted)]">{description}</p>
      {action}
    </div>
  );
}
