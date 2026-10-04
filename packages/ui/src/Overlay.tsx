import { useEffect, type HTMLAttributes, type ReactNode } from "react";

import { cn } from "./utils";
import { repairViewport } from "./viewportLock";

/**
 * Window-fixed modal scrim.
 * No backdrop-filter: WebKitGTK's compositor paints half the view or stays
 * black after a blur layer is created and destroyed.
 */
export function Overlay({
  children,
  className,
  align = "center",
  ...rest
}: HTMLAttributes<HTMLDivElement> & {
  children: ReactNode;
  align?: "center" | "start";
}) {
  useEffect(() => {
    const root = document.documentElement;
    const count = Number(root.dataset.novaOverlayCount ?? "0") + 1;
    root.dataset.novaOverlayCount = String(count);
    return () => {
      const left = Number(root.dataset.novaOverlayCount ?? "1") - 1;
      if (left <= 0) delete root.dataset.novaOverlayCount;
      else root.dataset.novaOverlayCount = String(left);
      window.requestAnimationFrame(() => repairViewport());
    };
  }, []);

  return (
    <div
      {...rest}
      className={cn(
        "nova-overlay fixed inset-0 z-50 flex h-full w-full overflow-hidden p-4",
        align === "center"
          ? "items-center justify-center"
          : "items-start justify-center pt-16",
        className,
      )}
    >
      {children}
    </div>
  );
}
