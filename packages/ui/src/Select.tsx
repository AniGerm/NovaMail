import type { SelectHTMLAttributes } from "react";
import { cn } from "./utils";

export type SelectProps = SelectHTMLAttributes<HTMLSelectElement>;

/** Themed native select — accent chrome instead of OS green defaults. */
export function Select({ className, children, ...props }: SelectProps) {
  return (
    <select
      className={cn(
        "nova-select w-full bg-[var(--nova-surface)] px-3 text-[var(--nova-ink)]",
        className,
      )}
      {...props}
    >
      {children}
    </select>
  );
}
