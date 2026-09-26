import type { ButtonHTMLAttributes, PropsWithChildren } from "react";
import { cn } from "./utils";

export type IconButtonProps = PropsWithChildren<
  ButtonHTMLAttributes<HTMLButtonElement> & {
    label: string;
  }
>;

export function IconButton({ className, label, children, ...props }: IconButtonProps) {
  return (
    <button
      aria-label={label}
      title={label}
      className={cn(
        "inline-flex h-11 w-11 items-center justify-center rounded-[var(--nova-radius-md)] text-[var(--nova-ink)] hover:bg-[var(--nova-surface-2)]",
        className,
      )}
      {...props}
    >
      {children}
    </button>
  );
}
