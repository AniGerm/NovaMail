import { useEffect, useRef } from "react";

interface PanelResizeHandleProps {
  /** Current width of the panel to the left of this handle. */
  value: number;
  min: number;
  max: number;
  onChange: (next: number) => void;
  label: string;
}

/** Vertical drag handle between main-view columns. */
export function PanelResizeHandle({
  value,
  min,
  max,
  onChange,
  label,
}: PanelResizeHandleProps) {
  const drag = useRef<{ startX: number; startValue: number } | null>(null);

  useEffect(() => {
    function onMove(event: PointerEvent) {
      if (!drag.current) return;
      const delta = event.clientX - drag.current.startX;
      const next = Math.min(
        max,
        Math.max(min, Math.round(drag.current.startValue + delta)),
      );
      onChange(next);
    }
    function onUp() {
      if (!drag.current) return;
      drag.current = null;
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
    }
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    window.addEventListener("pointercancel", onUp);
    return () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("pointercancel", onUp);
    };
  }, [max, min, onChange]);

  return (
    <div
      role="separator"
      aria-orientation="vertical"
      aria-label={label}
      aria-valuenow={value}
      aria-valuemin={min}
      aria-valuemax={max}
      tabIndex={0}
      className="group relative z-10 w-1.5 shrink-0 cursor-col-resize touch-none bg-transparent hover:bg-[color-mix(in_srgb,var(--nova-accent)_35%,transparent)] focus-visible:bg-[color-mix(in_srgb,var(--nova-accent)_45%,transparent)] focus-visible:outline-none"
      onPointerDown={(event) => {
        event.preventDefault();
        drag.current = { startX: event.clientX, startValue: value };
        document.body.style.cursor = "col-resize";
        document.body.style.userSelect = "none";
        (event.currentTarget as HTMLElement).setPointerCapture?.(event.pointerId);
      }}
      onKeyDown={(event) => {
        const step = event.shiftKey ? 24 : 8;
        if (event.key === "ArrowLeft") {
          event.preventDefault();
          onChange(Math.max(min, value - step));
        } else if (event.key === "ArrowRight") {
          event.preventDefault();
          onChange(Math.min(max, value + step));
        } else if (event.key === "Home") {
          event.preventDefault();
          onChange(min);
        } else if (event.key === "End") {
          event.preventDefault();
          onChange(max);
        }
      }}
    >
      <span
        aria-hidden
        className="pointer-events-none absolute inset-y-0 left-1/2 w-px -translate-x-1/2 bg-[var(--nova-border)] group-hover:bg-[var(--nova-accent)] group-focus-visible:bg-[var(--nova-accent)]"
      />
    </div>
  );
}
