import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import type { CalendarEventDto } from "@/shared/api/types";
import { useT } from "@/shared/i18n/useT";

const HOUR_HEIGHT = 52;
const SNAP_MINUTES = 15;
const SNAP_SECS = SNAP_MINUTES * 60;
const MIN_DURATION = 15 * 60;

export const DAY_HOURS = Array.from({ length: 24 }, (_, i) => i);

type DragMode = "move" | "resize-start" | "resize-end";

interface DragState {
  mode: DragMode;
  eventId: string;
  originY: number;
  originStartsAt: number;
  originEndsAt: number;
  dayStart: number;
  draftStartsAt: number;
  draftEndsAt: number;
}

export interface TimedCommit {
  id: string;
  startsAt: number;
  endsAt: number;
  /** Later same-day timed events that would overlap after the move/resize. */
  conflicts: CalendarEventDto[];
}

interface DayColumn {
  dayStart: number;
  events: CalendarEventDto[];
  isToday?: boolean;
}

interface TimedEventGridProps {
  days: DayColumn[];
  locale: string;
  /** Single-column (day) vs multi-column (week). */
  compact?: boolean;
  onCreateAt: (unixSeconds: number) => void;
  onOpenEvent: (ev: CalendarEventDto) => void;
  onCommitChange: (commit: TimedCommit) => void | Promise<void>;
}

function snapUnix(unix: number): number {
  return Math.round(unix / SNAP_SECS) * SNAP_SECS;
}

function clampToDay(unix: number, dayStart: number): number {
  const dayEnd = dayStart + 86400;
  return Math.min(Math.max(unix, dayStart), dayEnd - MIN_DURATION);
}

function eventEnd(ev: CalendarEventDto): number {
  return ev.endsAt ?? ev.startsAt + 3600;
}

function formatHour(hour: number): string {
  return `${String(hour).padStart(2, "0")}:00`;
}

function formatClock(unix: number, locale: string): string {
  return new Date(unix * 1000).toLocaleTimeString(
    locale === "de" ? "de-DE" : "en-US",
    { hour: "2-digit", minute: "2-digit" },
  );
}

export function TimedEventGrid({
  days,
  locale,
  compact = false,
  onCreateAt,
  onOpenEvent,
  onCommitChange,
}: TimedEventGridProps) {
  const t = useT();
  const gridRef = useRef<HTMLDivElement>(null);
  const [drag, setDrag] = useState<DragState | null>(null);
  const dragRef = useRef<DragState | null>(null);
  const movedRef = useRef(false);
  const didScrollRef = useRef(false);

  useEffect(() => {
    dragRef.current = drag;
  }, [drag]);

  // Start scrolled near the first timed event (or 07:00), while still showing 00:00–23:00.
  useEffect(() => {
    if (didScrollRef.current || !gridRef.current) return;
    const earliest = days
      .flatMap((d) => d.events)
      .filter((ev) => !ev.allDay)
      .reduce<number | null>((min, ev) => {
        const hour = new Date(ev.startsAt * 1000).getHours();
        return min == null ? hour : Math.min(min, hour);
      }, null);
    const targetHour = Math.max(0, (earliest ?? 7) - 1);
    gridRef.current.scrollTop = targetHour * HOUR_HEIGHT;
    didScrollRef.current = true;
  }, [days]);

  const timedByDay = useMemo(
    () =>
      days.map((day) => ({
        ...day,
        timed: day.events
          .filter((ev) => !ev.allDay)
          .slice()
          .sort((a, b) => a.startsAt - b.startsAt),
      })),
    [days],
  );

  const nowOverlay = useMemo(() => {
    const nowSec = Math.floor(Date.now() / 1000);
    const col = timedByDay.findIndex((d) => d.isToday);
    if (col < 0) return null;
    const day = timedByDay[col]!;
    if (nowSec < day.dayStart || nowSec >= day.dayStart + 86400) return null;
    const top = ((nowSec - day.dayStart) / 3600) * HOUR_HEIGHT;
    return { col, top, cols: timedByDay.length };
  }, [timedByDay]);

  const beginDrag = useCallback(
    (
      mode: DragMode,
      ev: CalendarEventDto,
      dayStart: number,
      clientY: number,
      pointerId: number,
      target: HTMLElement,
    ) => {
      target.setPointerCapture(pointerId);
      movedRef.current = false;
      const endsAt = eventEnd(ev);
      const next: DragState = {
        mode,
        eventId: ev.id,
        originY: clientY,
        originStartsAt: ev.startsAt,
        originEndsAt: endsAt,
        dayStart,
        draftStartsAt: ev.startsAt,
        draftEndsAt: endsAt,
      };
      dragRef.current = next;
      setDrag(next);
    },
    [],
  );

  const onPointerMove = useCallback((e: React.PointerEvent) => {
    const current = dragRef.current;
    if (!current) return;
    const deltaPx = e.clientY - current.originY;
    if (Math.abs(deltaPx) > 3) movedRef.current = true;
    const deltaSecs = Math.round(deltaPx / HOUR_HEIGHT) * 3600;
    // finer: use fractional hours then snap
    const deltaFine = (deltaPx / HOUR_HEIGHT) * 3600;
    const snappedDelta = Math.round(deltaFine / SNAP_SECS) * SNAP_SECS;

    let draftStartsAt = current.originStartsAt;
    let draftEndsAt = current.originEndsAt;
    const duration = current.originEndsAt - current.originStartsAt;

    if (current.mode === "move") {
      draftStartsAt = clampToDay(
        snapUnix(current.originStartsAt + snappedDelta),
        current.dayStart,
      );
      draftEndsAt = draftStartsAt + duration;
      if (draftEndsAt > current.dayStart + 86400) {
        draftEndsAt = current.dayStart + 86400;
        draftStartsAt = Math.max(
          current.dayStart,
          draftEndsAt - duration,
        );
      }
    } else if (current.mode === "resize-start") {
      draftStartsAt = snapUnix(current.originStartsAt + snappedDelta);
      draftStartsAt = Math.max(current.dayStart, draftStartsAt);
      draftStartsAt = Math.min(draftStartsAt, current.originEndsAt - MIN_DURATION);
      draftEndsAt = current.originEndsAt;
    } else {
      draftEndsAt = snapUnix(current.originEndsAt + snappedDelta);
      draftEndsAt = Math.min(current.dayStart + 86400, draftEndsAt);
      draftEndsAt = Math.max(draftEndsAt, current.originStartsAt + MIN_DURATION);
      draftStartsAt = current.originStartsAt;
    }

    // silence unused
    void deltaSecs;

    const next = { ...current, draftStartsAt, draftEndsAt };
    dragRef.current = next;
    setDrag(next);
  }, []);

  const finishDrag = useCallback(async () => {
    const current = dragRef.current;
    dragRef.current = null;
    setDrag(null);
    if (!current || !movedRef.current) return;
    if (
      current.draftStartsAt === current.originStartsAt &&
      current.draftEndsAt === current.originEndsAt
    ) {
      return;
    }

    const day = timedByDay.find((d) => d.dayStart === current.dayStart);
    const conflicts = (day?.timed ?? []).filter((ev) => {
      if (ev.id === current.eventId) return false;
      const end = eventEnd(ev);
      // Same-day events that start at/after the moved event and overlap the new window.
      const isLaterOrNext =
        ev.startsAt >= Math.min(current.originStartsAt, current.draftStartsAt);
      const overlaps =
        ev.startsAt < current.draftEndsAt && end > current.draftStartsAt;
      return isLaterOrNext && overlaps;
    });

    await onCommitChange({
      id: current.eventId,
      startsAt: current.draftStartsAt,
      endsAt: current.draftEndsAt,
      conflicts,
    });
  }, [onCommitChange, timedByDay]);

  const gutter = compact ? 48 : 56;
  const cols = timedByDay.length;

  return (
    <div
      ref={gridRef}
      className="relative max-h-[min(70dvh,900px)] overflow-y-auto"
      onPointerMove={onPointerMove}
      onPointerUp={() => void finishDrag()}
      onPointerCancel={() => {
        dragRef.current = null;
        setDrag(null);
      }}
    >
      <div
        className="relative"
        style={{
          display: "grid",
          gridTemplateColumns: `${gutter}px repeat(${cols}, minmax(0, 1fr))`,
          minHeight: 24 * HOUR_HEIGHT,
        }}
      >
        {/* Hour labels */}
        <div className="relative border-r border-[var(--nova-border)]">
          {DAY_HOURS.map((hour) => (
            <div
              key={hour}
              className="absolute right-1 text-right text-[10px] tabular-nums text-[var(--nova-ink-muted)]"
              style={{ top: hour * HOUR_HEIGHT - 6 }}
            >
              {formatHour(hour)}
            </div>
          ))}
        </div>

        {/* Day columns */}
        {timedByDay.map((day, colIndex) => (
          <div
            key={day.dayStart}
            className="relative border-r border-[var(--nova-border)] last:border-r-0"
            style={{ height: 24 * HOUR_HEIGHT }}
            onDoubleClick={(e) => {
              const rect = e.currentTarget.getBoundingClientRect();
              const y = e.clientY - rect.top;
              const secs = snapUnix(
                day.dayStart + Math.floor((y / HOUR_HEIGHT) * 3600),
              );
              onCreateAt(secs);
            }}
            onClick={(e) => {
              // Empty-slot click creates (ignore event chip clicks).
              if ((e.target as HTMLElement).closest("[data-cal-event]")) return;
              if (drag || movedRef.current) return;
              const rect = e.currentTarget.getBoundingClientRect();
              const y = e.clientY - rect.top;
              const secs = snapUnix(
                day.dayStart + Math.floor((y / HOUR_HEIGHT) * 3600),
              );
              onCreateAt(secs);
            }}
          >
            {DAY_HOURS.map((hour) => (
              <div
                key={hour}
                className="pointer-events-none absolute left-0 right-0 border-b border-[var(--nova-border)]"
                style={{ top: hour * HOUR_HEIGHT, height: HOUR_HEIGHT }}
              />
            ))}

            {day.timed.map((ev) => {
              const isDragging = drag?.eventId === ev.id;
              const startsAt = isDragging ? drag!.draftStartsAt : ev.startsAt;
              const endsAt = isDragging ? drag!.draftEndsAt : eventEnd(ev);
              const top =
                ((startsAt - day.dayStart) / 3600) * HOUR_HEIGHT + 1;
              const height = Math.max(
                22,
                ((endsAt - startsAt) / 3600) * HOUR_HEIGHT - 2,
              );
              return (
                <div
                  key={ev.id}
                  data-cal-event={ev.id}
                  className="absolute left-0.5 right-0.5 z-[1] overflow-hidden rounded-md text-white shadow-sm"
                  style={{
                    top,
                    height,
                    background: ev.color ?? "var(--nova-accent)",
                    opacity: isDragging ? 0.92 : 1,
                    cursor: drag?.mode === "move" ? "grabbing" : "grab",
                  }}
                  onClick={(e) => {
                    e.stopPropagation();
                    if (movedRef.current) return;
                    onOpenEvent(ev);
                  }}
                >
                  {/* Resize start */}
                  <div
                    className="absolute inset-x-0 top-0 z-[2] h-2 cursor-ns-resize"
                    title={t("calendarResizeStart")}
                    onPointerDown={(e) => {
                      e.stopPropagation();
                      e.preventDefault();
                      beginDrag(
                        "resize-start",
                        ev,
                        day.dayStart,
                        e.clientY,
                        e.pointerId,
                        e.currentTarget,
                      );
                    }}
                  />
                  <div
                    className="flex h-full flex-col px-1.5 py-1"
                    onPointerDown={(e) => {
                      if ((e.target as HTMLElement).closest("[data-resize]")) {
                        return;
                      }
                      e.stopPropagation();
                      beginDrag(
                        "move",
                        ev,
                        day.dayStart,
                        e.clientY,
                        e.pointerId,
                        e.currentTarget,
                      );
                    }}
                  >
                    <span
                      className={`truncate font-medium leading-tight ${compact ? "text-[10px]" : "text-xs"}`}
                    >
                      {formatClock(startsAt, locale)} {ev.title}
                    </span>
                    {!compact && height > 40 ? (
                      <span className="truncate text-[10px] opacity-90">
                        {formatClock(endsAt, locale)}
                        {ev.location ? ` · ${ev.location}` : ""}
                      </span>
                    ) : null}
                  </div>
                  {/* Resize end */}
                  <div
                    data-resize="end"
                    className="absolute inset-x-0 bottom-0 z-[2] h-2 cursor-ns-resize"
                    title={t("calendarResizeEnd")}
                    onPointerDown={(e) => {
                      e.stopPropagation();
                      e.preventDefault();
                      beginDrag(
                        "resize-end",
                        ev,
                        day.dayStart,
                        e.clientY,
                        e.pointerId,
                        e.currentTarget,
                      );
                    }}
                  />
                </div>
              );
            })}

            {nowOverlay && nowOverlay.col === colIndex ? (
              <div
                className="pointer-events-none absolute left-0 right-0 z-10 flex items-center"
                style={{ top: nowOverlay.top }}
                aria-hidden
              >
                <span className="h-2 w-2 shrink-0 rounded-full bg-[var(--nova-danger)]" />
                <span className="h-px flex-1 bg-[var(--nova-danger)]" />
              </div>
            ) : null}
          </div>
        ))}
      </div>
    </div>
  );
}

/** Cascade-push later events so they start after `afterEndsAt`, preserving duration. */
export function planPushBack(
  movedId: string,
  afterEndsAt: number,
  candidates: CalendarEventDto[],
): Array<{ id: string; startsAt: number; endsAt: number; title: string }> {
  let cursor = afterEndsAt;
  const sorted = candidates
    .filter((ev) => ev.id !== movedId)
    .slice()
    .sort((a, b) => a.startsAt - b.startsAt);
  const updates: Array<{
    id: string;
    startsAt: number;
    endsAt: number;
    title: string;
  }> = [];
  for (const ev of sorted) {
    const duration = eventEnd(ev) - ev.startsAt;
    if (ev.startsAt >= cursor) {
      cursor = Math.max(cursor, eventEnd(ev));
      continue;
    }
    const startsAt = cursor;
    const endsAt = startsAt + duration;
    updates.push({ id: ev.id, startsAt, endsAt, title: ev.title });
    cursor = endsAt;
  }
  return updates;
}
