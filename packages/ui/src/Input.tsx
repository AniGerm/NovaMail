import { forwardRef, type InputHTMLAttributes } from "react";
import { cn } from "./utils";

export type InputProps = InputHTMLAttributes<HTMLInputElement>;

export const Input = forwardRef<HTMLInputElement, InputProps>(
  function Input({ className, ...props }, ref) {
    return (
      <input
        ref={ref}
        className={cn(
          "h-11 w-full rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] bg-[var(--nova-surface)] px-3 text-[var(--nova-ink)] placeholder:text-[var(--nova-ink-muted)]",
          className,
        )}
        {...props}
      />
    );
  },
);
