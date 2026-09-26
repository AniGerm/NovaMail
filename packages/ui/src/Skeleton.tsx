import { cn } from "./utils";

export function Skeleton({ className }: { className?: string }) {
  return (
    <div
      className={cn(
        "animate-pulse rounded-[var(--nova-radius-sm)] bg-[var(--nova-surface-2)]",
        className,
      )}
    />
  );
}
