import type { ButtonHTMLAttributes, PropsWithChildren } from "react";
import { cn } from "./utils";

type Variant = "primary" | "secondary" | "ghost" | "danger";
type Size = "sm" | "md" | "lg";

export type ButtonProps = PropsWithChildren<
  ButtonHTMLAttributes<HTMLButtonElement> & {
    variant?: Variant;
    size?: Size;
  }
>;

const variants: Record<Variant, string> = {
  primary:
    "bg-[var(--nova-accent)] text-white dark:text-[#0e1114] hover:opacity-90",
  secondary:
    "bg-[var(--nova-surface-2)] text-[var(--nova-ink)] border border-[var(--nova-border)] hover:bg-[var(--nova-surface)]",
  ghost: "bg-transparent text-[var(--nova-ink)] hover:bg-[var(--nova-surface-2)]",
  danger: "bg-[var(--nova-danger)] text-white hover:opacity-90",
};

const sizes: Record<Size, string> = {
  sm: "h-9 px-3 text-sm rounded-[var(--nova-radius-sm)]",
  md: "h-11 px-4 text-sm rounded-[var(--nova-radius-md)]",
  lg: "h-12 px-5 text-base rounded-[var(--nova-radius-md)]",
};

export function Button({
  className,
  variant = "primary",
  size = "md",
  children,
  ...props
}: ButtonProps) {
  return (
    <button
      className={cn(
        "inline-flex min-w-11 items-center justify-center gap-2 font-medium transition-opacity disabled:opacity-50",
        variants[variant],
        sizes[size],
        className,
      )}
      {...props}
    >
      {children}
    </button>
  );
}
