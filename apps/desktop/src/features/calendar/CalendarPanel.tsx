import { useCallback, useEffect, useMemo, useState } from "react";
import { Button, Input } from "@novamail/ui";

import {
  planPushBack,
  TimedEventGrid,
  type TimedCommit,
} from "@/features/calendar/TimedEventGrid";
import { api } from "@/shared/api/client";
import type {
  AppError,
  CalendarAccountDto,
  CalendarCollectionDto,
  CalendarEventDto,
  CalendarInvitationDto,
  CalendarTaskDto,
  InvitationResponse,
} from "@/shared/api/types";
import { useT } from "@/shared/i18n/useT";
import { useUiStore } from "@/shared/store/uiStore";

type ViewMode = "month" | "week" | "day";
type PanelTab = "schedule" | "inbox" | "tasks" | "manage";

const REMINDER_OPTIONS = [0, 5, 15, 30, 60] as const;
const COLOR_PALETTE = [
  "#1e3a5f",
  "#0f766e",
  "#166534",
  "#b45309",
  "#be123c",
  "#334155",
];

interface EventDraft {
  id?: string;
  title: string;
  startsAt: number;
  endsAt: number;
  location: string;
  description: string;
  allDay: boolean;
  collectionId: string;
  reminderMinutes: number;
}

export function CalendarPanel() {
  const t = useT();
  const locale = useUiStore((s) => s.locale);
  const loc = locale === "de" ? "de-DE" : "en-US";
  const [view, setView] = useState<ViewMode>("month");
  const [tab, setTab] = useState<PanelTab>("schedule");
  const [anchor, setAnchor] = useState(() => startOfDay(Date.now()));
  const [events, setEvents] = useState<CalendarEventDto[]>([]);
  const [tasks, setTasks] = useState<CalendarTaskDto[]>([]);
  const [accounts, setAccounts] = useState<CalendarAccountDto[]>([]);
  const [collections, setCollections] = useState<CalendarCollectionDto[]>([]);
  const [invites, setInvites] = useState<CalendarInvitationDto[]>([]);
  const [name, setName] = useState("");
  const [url, setUrl] = useState("");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [discovered, setDiscovered] = useState<
    { href: string; displayName: string }[]
  >([]);
  const [newTaskTitle, setNewTaskTitle] = useState("");
  const [editingTaskId, setEditingTaskId] = useState<string | null>(null);
  const [editingTaskTitle, setEditingTaskTitle] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [draft, setDraft] = useState<EventDraft | null>(null);
  const [saving, setSaving] = useState(false);

  const range = useMemo(() => {
    if (view === "month") {
      const monthStart = startOfMonth(anchor);
      const gridStart = startOfWeekMonday(monthStart);
      const from = Math.floor(gridStart / 1000);
      const to = from + 42 * 86400 - 1;
      return { from, to, days: 42, gridStart };
    }
    if (view === "week") {
      const weekStart = startOfWeekMonday(anchor);
      const from = Math.floor(weekStart / 1000);
      return { from, to: from + 7 * 86400 - 1, days: 7, gridStart: weekStart };
    }
    const day = startOfDay(anchor);
    const from = Math.floor(day / 1000);
    return { from, to: from + 86399, days: 1, gridStart: day };
  }, [anchor, view]);

  const defaultCollection = useMemo(
    () => collections.find((c) => c.isDefault) ?? collections[0] ?? null,
    [collections],
  );

  const refresh = useCallback(async () => {
    try {
      const [e, ta, ac, cols, inv] = await Promise.all([
        api.calendarEventsList(range.from, range.to),
        api.calendarTasksList(true),
        api.calendarAccountsList(),
        api.calendarCollectionsList(),
        api.calendarInvitationsList(true),
      ]);
      setEvents(e);
      setTasks(ta);
      setAccounts(ac);
      setCollections(cols);
      setInvites(inv);
      setError(null);
    } catch (err) {
      setError((err as AppError).message);
    }
  }, [range.from, range.to]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const periodTitle = useMemo(() => {
    if (view === "month") {
      return new Date(anchor).toLocaleDateString(loc, {
        month: "long",
        year: "numeric",
      });
    }
    if (view === "week") {
      const end = range.gridStart + 6 * 86400000;
      const a = new Date(range.gridStart).toLocaleDateString(loc, {
        day: "numeric",
        month: "short",
      });
      const b = new Date(end).toLocaleDateString(loc, {
        day: "numeric",
        month: "short",
        year: "numeric",
      });
      return `${a} – ${b}`;
    }
    return new Date(anchor).toLocaleDateString(loc, {
      weekday: "long",
      day: "numeric",
      month: "long",
      year: "numeric",
    });
  }, [anchor, loc, range.gridStart, view]);

  const days = useMemo(() => {
    return Array.from({ length: range.days }, (_, i) => {
      const start = range.gridStart + i * 86400000;
      const dayStart = Math.floor(start / 1000);
      const dayEnd = dayStart + 86399;
      return {
        start,
        dayStart,
        inMonth:
          view !== "month" ||
          new Date(start).getMonth() === new Date(anchor).getMonth(),
        isToday: startOfDay(Date.now()) === startOfDay(start),
        events: events.filter(
          (ev) =>
            ev.startsAt <= dayEnd &&
            (ev.endsAt == null || ev.endsAt >= dayStart),
        ),
      };
    });
  }, [anchor, events, range.days, range.gridStart, view]);

  const weekdayLabels = t("calendarWeekdays").split(",");

  const shiftPeriod = (dir: -1 | 1) => {
    const d = new Date(anchor);
    if (view === "month") d.setMonth(d.getMonth() + dir);
    else if (view === "week") d.setDate(d.getDate() + dir * 7);
    else d.setDate(d.getDate() + dir);
    setAnchor(startOfDay(d.getTime()));
  };

  const openNewEvent = (dayStartMs?: number) => {
    const base = dayStartMs ?? Date.now();
    const raw = Math.floor(base / 1000);
    // Grid clicks pass an exact slot; the toolbar button rounds to the next hour.
    const startsAt =
      dayStartMs != null ? raw : raw - (raw % 3600) + 3600;
    setDraft({
      title: "",
      startsAt,
      endsAt: startsAt + 3600,
      location: "",
      description: "",
      allDay: false,
      collectionId: defaultCollection?.id ?? "",
      reminderMinutes: 15,
    });
  };

  const openEditEvent = (ev: CalendarEventDto) => {
    setDraft({
      id: ev.id,
      title: ev.title,
      startsAt: ev.startsAt,
      endsAt: ev.endsAt ?? ev.startsAt + 3600,
      location: ev.location ?? "",
      description: ev.description ?? "",
      allDay: ev.allDay,
      collectionId: ev.collectionId ?? defaultCollection?.id ?? "",
      reminderMinutes: ev.reminders?.[0]?.minutes ?? 15,
    });
  };

  const saveDraft = async () => {
    if (!draft || !draft.title.trim()) return;
    setSaving(true);
    try {
      await api.calendarEventsUpsert({
        id: draft.id,
        collectionId: draft.collectionId || null,
        title: draft.title.trim(),
        startsAt: draft.startsAt,
        endsAt: draft.endsAt,
        location: draft.location.trim() || null,
        description: draft.description.trim() || null,
        allDay: draft.allDay,
        reminders: [{ minutes: draft.reminderMinutes }],
        status: "confirmed",
      });
      setDraft(null);
      setStatus(t("calendarSaved"));
      await refresh();
    } catch (err) {
      setError((err as AppError).message);
    } finally {
      setSaving(false);
    }
  };

  const deleteDraft = async () => {
    if (!draft?.id) {
      setDraft(null);
      return;
    }
    setSaving(true);
    try {
      await api.calendarEventsDelete(draft.id);
      setDraft(null);
      await refresh();
    } catch (err) {
      setError((err as AppError).message);
    } finally {
      setSaving(false);
    }
  };

  const respondInvite = async (id: string, response: InvitationResponse) => {
    try {
      await api.calendarInvitationsRespond(id, response);
      setStatus(t("calendarInviteUpdated"));
      await refresh();
    } catch (err) {
      setError((err as AppError).message);
    }
  };

  const createTask = async () => {
    const title = newTaskTitle.trim();
    if (!title) return;
    try {
      await api.calendarTasksUpsert({
        title,
        dueAt: Math.floor(Date.now() / 1000) + 86400,
      });
      setNewTaskTitle("");
      setStatus(t("calendarTaskSaved"));
      await refresh();
    } catch (err) {
      setError((err as AppError).message);
    }
  };

  const toggleTaskCompleted = async (task: CalendarTaskDto) => {
    try {
      await api.calendarTasksUpsert({
        id: task.id,
        title: task.title,
        dueAt: task.dueAt,
        completed: !task.completed,
        notes: task.notes,
      });
      await refresh();
    } catch (err) {
      setError((err as AppError).message);
    }
  };

  const beginRenameTask = (task: CalendarTaskDto) => {
    setEditingTaskId(task.id);
    setEditingTaskTitle(task.title);
  };

  const saveRenameTask = async (task: CalendarTaskDto) => {
    const title = editingTaskTitle.trim();
    if (!title || title === task.title) {
      setEditingTaskId(null);
      setEditingTaskTitle("");
      return;
    }
    try {
      await api.calendarTasksUpsert({
        id: task.id,
        title,
        dueAt: task.dueAt,
        completed: task.completed,
        notes: task.notes,
      });
      setEditingTaskId(null);
      setEditingTaskTitle("");
      setStatus(t("calendarTaskSaved"));
      await refresh();
    } catch (err) {
      setError((err as AppError).message);
    }
  };

  const deleteTask = async (id: string) => {
    try {
      await api.calendarTasksDelete(id);
      if (editingTaskId === id) {
        setEditingTaskId(null);
        setEditingTaskTitle("");
      }
      setStatus(t("calendarTaskDeleted"));
      await refresh();
    } catch (err) {
      setError((err as AppError).message);
    }
  };

  const [conflictPrompt, setConflictPrompt] = useState<{
    primary: TimedCommit;
    push: Array<{ id: string; startsAt: number; endsAt: number; title: string }>;
  } | null>(null);

  const persistEventTimes = useCallback(
    async (
      updates: Array<{
        id: string;
        startsAt: number;
        endsAt: number;
      }>,
    ) => {
      for (const update of updates) {
        const ev = events.find((item) => item.id === update.id);
        if (!ev) continue;
        await api.calendarEventsUpsert({
          id: ev.id,
          collectionId: ev.collectionId,
          calendarAccountId: ev.calendarAccountId,
          title: ev.title,
          startsAt: update.startsAt,
          endsAt: update.endsAt,
          location: ev.location,
          description: ev.description,
          allDay: ev.allDay,
          reminders: ev.reminders,
          status: ev.status,
        });
      }
      await refresh();
    },
    [events, refresh],
  );

  const applyTimedCommit = useCallback(
    async (commit: TimedCommit, pushOthers: boolean) => {
      const updates = [
        {
          id: commit.id,
          startsAt: commit.startsAt,
          endsAt: commit.endsAt,
        },
      ];
      if (pushOthers && commit.conflicts.length > 0) {
        updates.push(
          ...planPushBack(commit.id, commit.endsAt, commit.conflicts),
        );
      }
      try {
        await persistEventTimes(updates);
        setStatus(
          pushOthers && commit.conflicts.length > 0
            ? t("calendarOptimized")
            : t("calendarSaved"),
        );
      } catch (err) {
        setError((err as AppError).message);
      }
    },
    [persistEventTimes, t],
  );

  const handleTimedCommit = useCallback(
    async (commit: TimedCommit) => {
      if (commit.conflicts.length === 0) {
        await applyTimedCommit(commit, false);
        return;
      }
      const push = planPushBack(commit.id, commit.endsAt, commit.conflicts);
      if (push.length === 0) {
        await applyTimedCommit(commit, false);
        return;
      }
      setConflictPrompt({ primary: commit, push });
    },
    [applyTimedCommit],
  );

  const segmentBtn = (active: boolean) =>
    active
      ? "rounded-[calc(var(--nova-radius-md)-2px)] bg-[var(--nova-surface)] px-3 py-1.5 text-sm font-medium text-[var(--nova-ink)] shadow-sm"
      : "px-3 py-1.5 text-sm text-[var(--nova-ink-muted)] hover:text-[var(--nova-ink)]";

  return (
    <section className="relative flex h-full flex-col overflow-hidden">
      {/* Hero header — period name is the brand signal, not a second "Kalender" */}
      <header className="nova-fade-in border-b border-[var(--nova-border)] px-6 pb-4 pt-5">
        <div className="flex flex-wrap items-start justify-between gap-4">
          <div className="min-w-0">
            <p className="text-xs font-medium uppercase tracking-[0.14em] text-[var(--nova-ink-muted)]">
              {t("calendar")}
            </p>
            <h1 className="mt-1 font-[family-name:var(--nova-font-display)] text-3xl tracking-tight capitalize">
              {periodTitle}
            </h1>
          </div>
          <div className="flex flex-wrap items-center gap-2">
            <nav
              aria-label={t("calendar")}
              className="flex rounded-[var(--nova-radius-md)] bg-[color-mix(in_srgb,var(--nova-surface-2)_80%,transparent)] p-0.5"
            >
              {(
                [
                  ["schedule", t("calendarSchedule")],
                  [
                    "inbox",
                    invites.length
                      ? `${t("calendarInbox")} · ${invites.length}`
                      : t("calendarInbox"),
                  ],
                  [
                    "tasks",
                    tasks.filter((task) => !task.completed).length
                      ? `${t("calendarTasksTitle")} · ${tasks.filter((task) => !task.completed).length}`
                      : t("calendarTasksTitle"),
                  ],
                  ["manage", t("calendarManage")],
                ] as const
              ).map(([key, label]) => (
                <button
                  key={key}
                  type="button"
                  onClick={() => setTab(key)}
                  className={segmentBtn(tab === key)}
                >
                  {label}
                </button>
              ))}
            </nav>
            <Button size="sm" onClick={() => openNewEvent()}>
              {t("calendarNewEvent")}
            </Button>
          </div>
        </div>

        {tab === "schedule" ? (
          <div className="mt-4 flex flex-wrap items-center gap-3">
            <div className="flex items-center gap-1">
              <button
                type="button"
                aria-label={t("calendarPrev")}
                onClick={() => shiftPeriod(-1)}
                className="flex h-9 w-9 items-center justify-center rounded-full text-[var(--nova-ink-muted)] transition-colors hover:bg-[var(--nova-accent-soft)] hover:text-[var(--nova-ink)]"
              >
                ←
              </button>
              <button
                type="button"
                onClick={() => setAnchor(startOfDay(Date.now()))}
                className="h-9 rounded-full px-3 text-sm font-medium text-[var(--nova-accent)] transition-colors hover:bg-[var(--nova-accent-soft)]"
              >
                {t("calendarToday")}
              </button>
              <button
                type="button"
                aria-label={t("calendarNext")}
                onClick={() => shiftPeriod(1)}
                className="flex h-9 w-9 items-center justify-center rounded-full text-[var(--nova-ink-muted)] transition-colors hover:bg-[var(--nova-accent-soft)] hover:text-[var(--nova-ink)]"
              >
                →
              </button>
            </div>
            <div className="flex rounded-[var(--nova-radius-md)] bg-[color-mix(in_srgb,var(--nova-surface-2)_80%,transparent)] p-0.5">
              {(
                [
                  ["month", t("calendarMonth")],
                  ["week", t("calendarWeek")],
                  ["day", t("calendarDay")],
                ] as const
              ).map(([key, label]) => (
                <button
                  key={key}
                  type="button"
                  onClick={() => {
                    setView(key);
                    if (key === "week") {
                      setAnchor(startOfWeekMonday(anchor));
                    }
                  }}
                  className={segmentBtn(view === key)}
                >
                  {label}
                </button>
              ))}
            </div>
          </div>
        ) : null}
      </header>

      {(error || status) && (
        <p
          className={`px-6 py-2 text-sm ${error ? "text-[var(--nova-danger)]" : "text-[var(--nova-ink-muted)]"}`}
          role="status"
        >
          {error ?? status}
        </p>
      )}

      <div className="min-h-0 flex-1 overflow-y-auto px-6 py-5">
        {tab === "schedule" ? (
          <div
            key={`${view}-${range.from}`}
            className={`nova-fade-in grid gap-6 ${collections.some((c) => c.isVisible) ? "xl:grid-cols-[1fr_220px]" : ""}`}
          >
            <div className="min-w-0">
              {view === "month" ? (
                <div className="overflow-hidden rounded-[var(--nova-radius-lg)] border border-[var(--nova-border)] bg-[color-mix(in_srgb,var(--nova-surface)_88%,transparent)]">
                  <div className="grid grid-cols-7 border-b border-[var(--nova-border)]">
                    {weekdayLabels.map((d) => (
                      <div
                        key={d}
                        className="px-2 py-2.5 text-center text-[11px] font-semibold uppercase tracking-wider text-[var(--nova-ink-muted)]"
                      >
                        {d}
                      </div>
                    ))}
                  </div>
                  <div className="grid grid-cols-7">
                    {days.map((day) => {
                      const dateNum = new Date(day.start).getDate();
                      return (
                        <button
                          key={day.start}
                          type="button"
                          onClick={() => openNewEvent(day.start)}
                          className={`min-h-[104px] border-b border-r border-[var(--nova-border)] p-1.5 text-left transition-colors last:border-r-0 hover:bg-[var(--nova-accent-soft)] ${
                            day.inMonth ? "" : "bg-[color-mix(in_srgb,var(--nova-surface-2)_45%,transparent)]"
                          }`}
                        >
                          <span
                            className={`mb-1 inline-flex h-7 min-w-7 items-center justify-center rounded-full px-1.5 text-sm ${
                              day.isToday
                                ? "bg-[var(--nova-accent)] font-semibold text-white"
                                : day.inMonth
                                  ? "font-medium text-[var(--nova-ink)]"
                                  : "text-[var(--nova-ink-muted)]"
                            }`}
                          >
                            {dateNum}
                          </span>
                          <ul className="space-y-0.5">
                            {day.events.slice(0, 3).map((ev) => (
                              <li key={ev.id}>
                                <span
                                  role="link"
                                  tabIndex={0}
                                  onClick={(e) => {
                                    e.stopPropagation();
                                    openEditEvent(ev);
                                  }}
                                  onKeyDown={(e) => {
                                    if (e.key === "Enter") {
                                      e.stopPropagation();
                                      openEditEvent(ev);
                                    }
                                  }}
                                  className="block truncate rounded px-1 py-0.5 text-[11px] font-medium text-white"
                                  style={{
                                    background: ev.color ?? "var(--nova-accent)",
                                  }}
                                >
                                  {!ev.allDay
                                    ? `${formatTime(ev.startsAt, locale)} `
                                    : ""}
                                  {ev.title}
                                </span>
                              </li>
                            ))}
                            {day.events.length > 3 ? (
                              <li className="px-1 text-[10px] text-[var(--nova-ink-muted)]">
                                +{day.events.length - 3}
                              </li>
                            ) : null}
                          </ul>
                        </button>
                      );
                    })}
                  </div>
                </div>
              ) : null}

              {view === "week" ? (
                <div className="overflow-hidden rounded-[var(--nova-radius-lg)] border border-[var(--nova-border)] bg-[color-mix(in_srgb,var(--nova-surface)_88%,transparent)]">
                  <div className="grid grid-cols-[48px_repeat(7,minmax(0,1fr))] border-b border-[var(--nova-border)]">
                    <div aria-hidden className="border-r border-[var(--nova-border)]" />
                    {days.map((day) => (
                      <button
                        key={`head-${day.start}`}
                        type="button"
                        onClick={() => {
                          setView("day");
                          setAnchor(startOfDay(day.start));
                        }}
                        className="flex flex-col items-center gap-0.5 border-r border-[var(--nova-border)] px-1 py-2.5 last:border-r-0 transition-colors hover:bg-[var(--nova-accent-soft)]"
                      >
                        <span className="text-[11px] font-semibold uppercase tracking-wider text-[var(--nova-ink-muted)]">
                          {new Date(day.start).toLocaleDateString(loc, {
                            weekday: "short",
                          })}
                        </span>
                        <span
                          className={`flex h-8 w-8 items-center justify-center rounded-full text-sm ${
                            day.isToday
                              ? "bg-[var(--nova-accent)] font-semibold text-white"
                              : "font-medium"
                          }`}
                        >
                          {new Date(day.start).getDate()}
                        </span>
                      </button>
                    ))}
                  </div>
                  <div className="grid grid-cols-[48px_repeat(7,minmax(0,1fr))] border-b border-[var(--nova-border)]">
                    <div
                      className="border-r border-[var(--nova-border)] px-1 py-1 text-[10px] uppercase tracking-wide text-[var(--nova-ink-muted)]"
                      aria-hidden
                    />
                    {days.map((day) => {
                      const allDay = day.events.filter((ev) => ev.allDay);
                      return (
                        <div
                          key={`allday-${day.start}`}
                          className="min-h-[36px] space-y-0.5 border-r border-[var(--nova-border)] p-1 last:border-r-0"
                        >
                          {allDay.map((ev) => (
                            <button
                              key={ev.id}
                              type="button"
                              onClick={() => openEditEvent(ev)}
                              className="block w-full truncate rounded px-1 py-0.5 text-left text-[10px] font-medium text-white"
                              style={{
                                background: ev.color ?? "var(--nova-accent)",
                              }}
                            >
                              {ev.title}
                            </button>
                          ))}
                        </div>
                      );
                    })}
                  </div>
                  <TimedEventGrid
                    days={days.map((day) => ({
                      dayStart: day.dayStart,
                      events: day.events,
                      isToday: day.isToday,
                    }))}
                    locale={locale}
                    compact
                    onCreateAt={(unix) => openNewEvent(unix * 1000)}
                    onOpenEvent={openEditEvent}
                    onCommitChange={handleTimedCommit}
                  />
                </div>
              ) : null}

              {view === "day" ? (
                <div className="overflow-hidden rounded-[var(--nova-radius-lg)] border border-[var(--nova-border)] bg-[color-mix(in_srgb,var(--nova-surface)_88%,transparent)]">
                  {(days[0]?.events ?? [])
                    .filter((ev) => ev.allDay)
                    .map((ev) => (
                      <button
                        key={ev.id}
                        type="button"
                        onClick={() => openEditEvent(ev)}
                        className="mx-3 mt-3 block w-[calc(100%-1.5rem)] rounded-md px-3 py-1.5 text-left text-sm text-white"
                        style={{ background: ev.color ?? "var(--nova-accent)" }}
                      >
                        {ev.title}
                        {ev.location ? ` · ${ev.location}` : ""}
                      </button>
                    ))}
                  <div className="mt-2">
                    <TimedEventGrid
                      days={[
                        {
                          dayStart:
                            days[0]?.dayStart ?? Math.floor(anchor / 1000),
                          events: days[0]?.events ?? [],
                          isToday: startOfDay(Date.now()) === startOfDay(anchor),
                        },
                      ]}
                      locale={locale}
                      onCreateAt={(unix) => openNewEvent(unix * 1000)}
                      onOpenEvent={openEditEvent}
                      onCommitChange={handleTimedCommit}
                    />
                  </div>
                </div>
              ) : null}
            </div>

            {collections.some((c) => c.isVisible) ? (
              <aside className="space-y-4">
                <section>
                  <h2 className="mb-2 text-xs font-semibold uppercase tracking-[0.12em] text-[var(--nova-ink-muted)]">
                    {t("calendarManageTitle")}
                  </h2>
                  <ul className="space-y-1.5">
                    {collections
                      .filter((c) => c.isVisible)
                      .map((c) => (
                        <li
                          key={c.id}
                          className="flex items-center gap-2 text-sm"
                        >
                          <span
                            className="h-2.5 w-2.5 shrink-0 rounded-full"
                            style={{ background: c.color }}
                          />
                          <span className="truncate">{c.displayName}</span>
                          {c.isDefault ? (
                            <span className="text-[10px] text-[var(--nova-accent)]">
                              {t("calendarMain")}
                            </span>
                          ) : null}
                        </li>
                      ))}
                  </ul>
                </section>
              </aside>
            ) : null}
          </div>
        ) : null}

        {tab === "inbox" ? (
          <div className="nova-slide-in mx-auto max-w-xl space-y-4">
            <div>
              <h2 className="font-[family-name:var(--nova-font-display)] text-2xl tracking-tight">
                {t("calendarInbox")}
              </h2>
              <p className="mt-1 text-sm text-[var(--nova-ink-muted)]">
                {t("calendarInboxHint")}
              </p>
            </div>
            {invites.length === 0 ? (
              <p className="text-sm text-[var(--nova-ink-muted)]">
                {t("calendarInboxEmpty")}
              </p>
            ) : (
              invites.map((inv) => (
                <article
                  key={inv.id}
                  className="rounded-[var(--nova-radius-lg)] border border-[var(--nova-border)] bg-[color-mix(in_srgb,var(--nova-surface)_90%,transparent)] px-5 py-4"
                >
                  <h3 className="text-base font-medium">{inv.title}</h3>
                  <p className="mt-1 text-sm text-[var(--nova-ink-muted)]">
                    {formatDateTime(inv.startsAt, locale)}
                    {inv.location ? ` · ${inv.location}` : ""}
                  </p>
                  {inv.organizer ? (
                    <p className="mt-0.5 text-sm text-[var(--nova-ink-muted)]">
                      {inv.organizer}
                    </p>
                  ) : null}
                  <div className="mt-4 flex flex-wrap gap-2">
                    <Button
                      size="sm"
                      onClick={() => void respondInvite(inv.id, "accept")}
                    >
                      {t("calendarAccept")}
                    </Button>
                    <Button
                      size="sm"
                      variant="secondary"
                      onClick={() => void respondInvite(inv.id, "tentative")}
                    >
                      {t("calendarMaybe")}
                    </Button>
                    <Button
                      size="sm"
                      variant="ghost"
                      onClick={() => void respondInvite(inv.id, "decline")}
                    >
                      {t("calendarDecline")}
                    </Button>
                  </div>
                </article>
              ))
            )}
          </div>
        ) : null}

        {tab === "tasks" ? (
          <div className="nova-slide-in mx-auto max-w-xl space-y-4">
            <div>
              <h2 className="font-[family-name:var(--nova-font-display)] text-2xl tracking-tight">
                {t("calendarTasksTitle")}
              </h2>
              <p className="mt-1 text-sm text-[var(--nova-ink-muted)]">
                {t("calendarTasksHint")}
              </p>
            </div>
            <div className="flex gap-2">
              <Input
                value={newTaskTitle}
                onChange={(e) => setNewTaskTitle(e.target.value)}
                placeholder={t("taskTitle")}
                onKeyDown={(e) => {
                  if (e.key === "Enter") void createTask();
                }}
              />
              <Button
                size="sm"
                variant="secondary"
                onClick={() => void createTask()}
              >
                {t("add")}
              </Button>
            </div>
            {tasks.length === 0 ? (
              <p className="text-sm text-[var(--nova-ink-muted)]">
                {t("calendarTasksEmpty")}
              </p>
            ) : (
              <ul className="space-y-2">
                {tasks.map((task) => {
                  const renaming = editingTaskId === task.id;
                  return (
                    <li
                      key={task.id}
                      className="flex items-start gap-3 rounded-[var(--nova-radius-lg)] border border-[var(--nova-border)] bg-[color-mix(in_srgb,var(--nova-surface)_90%,transparent)] px-4 py-3"
                    >
                      <input
                        type="checkbox"
                        className="mt-1.5"
                        checked={task.completed}
                        onChange={() => void toggleTaskCompleted(task)}
                        aria-label={task.title}
                      />
                      <div className="min-w-0 flex-1">
                        {renaming ? (
                          <Input
                            autoFocus
                            value={editingTaskTitle}
                            onChange={(e) =>
                              setEditingTaskTitle(e.target.value)
                            }
                            onKeyDown={(e) => {
                              if (e.key === "Enter") {
                                e.preventDefault();
                                void saveRenameTask(task);
                              } else if (e.key === "Escape") {
                                setEditingTaskId(null);
                                setEditingTaskTitle("");
                              }
                            }}
                            onBlur={() => void saveRenameTask(task)}
                          />
                        ) : (
                          <button
                            type="button"
                            className={`block w-full truncate text-left text-sm ${
                              task.completed
                                ? "text-[var(--nova-ink-muted)] line-through"
                                : "text-[var(--nova-ink)]"
                            }`}
                            onClick={() => beginRenameTask(task)}
                            title={t("calendarTaskRename")}
                          >
                            {task.title}
                          </button>
                        )}
                        {task.dueAt != null ? (
                          <p className="mt-1 text-xs text-[var(--nova-ink-muted)]">
                            {formatDateTime(task.dueAt, locale)}
                          </p>
                        ) : null}
                      </div>
                      <div className="flex shrink-0 gap-1">
                        {!renaming ? (
                          <Button
                            size="sm"
                            variant="ghost"
                            onClick={() => beginRenameTask(task)}
                          >
                            {t("calendarTaskRename")}
                          </Button>
                        ) : null}
                        <Button
                          size="sm"
                          variant="ghost"
                          onClick={() => void deleteTask(task.id)}
                        >
                          {t("delete")}
                        </Button>
                      </div>
                    </li>
                  );
                })}
              </ul>
            )}
          </div>
        ) : null}

        {tab === "manage" ? (
          <div className="nova-fade-in mx-auto max-w-2xl space-y-10">
            <section>
              <h2 className="font-[family-name:var(--nova-font-display)] text-2xl tracking-tight">
                {t("calendarManageTitle")}
              </h2>
              <ul className="mt-4 space-y-2">
                {collections.map((col) => (
                  <li
                    key={col.id}
                    className="flex flex-wrap items-center gap-3 rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] px-3 py-2.5"
                  >
                    <input
                      type="color"
                      value={col.color}
                      onChange={(e) => {
                        void api
                          .calendarCollectionsUpsert({
                            id: col.id,
                            calendarAccountId: col.calendarAccountId,
                            href: col.href,
                            displayName: col.displayName,
                            color: e.target.value,
                            isVisible: col.isVisible,
                            isDefault: col.isDefault,
                          })
                          .then(refresh)
                          .catch((err) => setError((err as AppError).message));
                      }}
                      className="h-8 w-8 cursor-pointer rounded border-0 bg-transparent"
                      aria-label={t("calendarColor")}
                    />
                    <div className="min-w-0 flex-1">
                      <p className="truncate text-sm font-medium">
                        {col.displayName}
                        {col.isDefault ? (
                          <span className="ml-2 text-xs text-[var(--nova-accent)]">
                            {t("calendarMain")}
                          </span>
                        ) : null}
                      </p>
                      <p className="truncate text-xs text-[var(--nova-ink-muted)]">
                        {col.calendarAccountId
                          ? t("caldavWriteback")
                          : t("calendarLocal")}
                      </p>
                    </div>
                    <label className="flex items-center gap-1.5 text-xs">
                      <input
                        type="checkbox"
                        checked={col.isVisible}
                        onChange={() => {
                          void api
                            .calendarCollectionsUpsert({
                              id: col.id,
                              calendarAccountId: col.calendarAccountId,
                              href: col.href,
                              displayName: col.displayName,
                              color: col.color,
                              isVisible: !col.isVisible,
                              isDefault: col.isDefault,
                            })
                            .then(refresh)
                            .catch((err) =>
                              setError((err as AppError).message),
                            );
                        }}
                      />
                      {t("calendarVisible")}
                    </label>
                    {!col.isDefault ? (
                      <Button
                        size="sm"
                        variant="secondary"
                        onClick={() => {
                          void api
                            .calendarCollectionsSetDefault(col.id)
                            .then(refresh)
                            .catch((err) =>
                              setError((err as AppError).message),
                            );
                        }}
                      >
                        {t("calendarSetMain")}
                      </Button>
                    ) : null}
                    <Button
                      size="sm"
                      variant="ghost"
                      disabled={collections.length <= 1}
                      onClick={() => {
                        if (
                          !window.confirm(
                            t("calendarDeleteConfirm", {
                              name: col.displayName,
                            }),
                          )
                        ) {
                          return;
                        }
                        void api
                          .calendarCollectionsDelete(col.id)
                          .then(() => {
                            setStatus(t("calendarDeleted"));
                            return refresh();
                          })
                          .catch((err) =>
                            setError((err as AppError).message),
                          );
                      }}
                    >
                      {t("delete")}
                    </Button>
                  </li>
                ))}
              </ul>
              <div className="mt-3 flex flex-wrap items-center gap-2">
                <span className="text-xs text-[var(--nova-ink-muted)]">
                  {t("calendarNewLocal")}
                </span>
                {COLOR_PALETTE.map((c) => (
                  <button
                    key={c}
                    type="button"
                    title={c}
                    className="h-6 w-6 rounded-full border border-[var(--nova-border)] transition-transform hover:scale-110"
                    style={{ background: c }}
                    onClick={() => {
                      void api
                        .calendarCollectionsUpsert({
                          displayName: t("calendarNewLocal"),
                          color: c,
                          isVisible: true,
                          isDefault: false,
                        })
                        .then(refresh)
                        .catch((err) => setError((err as AppError).message));
                    }}
                  />
                ))}
              </div>
            </section>

            <section>
              <h2 className="font-[family-name:var(--nova-font-display)] text-2xl tracking-tight">
                {t("calendarAccountsTitle")}
              </h2>
              <div className="mt-4 grid gap-2 sm:grid-cols-2">
                <Input
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder={t("caldavName")}
                />
                <Input
                  value={url}
                  onChange={(e) => setUrl(e.target.value)}
                  placeholder={t("caldavUrl")}
                />
                <Input
                  value={username}
                  onChange={(e) => setUsername(e.target.value)}
                  placeholder={t("username")}
                />
                <Input
                  type="password"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  placeholder={t("password")}
                />
              </div>
              <div className="mt-3 flex flex-wrap gap-2">
                <Button
                  size="sm"
                  variant="secondary"
                  onClick={() => {
                    void api
                      .calendarDiscover({
                        caldavUrl: url,
                        username,
                        password,
                      })
                      .then((cols) => {
                        setDiscovered(cols);
                        if (cols[0]) setUrl(cols[0].href);
                        setStatus(
                          `${t("caldavCollections")}: ${cols.map((c) => c.displayName).join(", ")}`,
                        );
                      })
                      .catch((err) => setError((err as AppError).message));
                  }}
                >
                  {t("caldavDiscover")}
                </Button>
                <Button
                  size="sm"
                  onClick={() => {
                    void api
                      .calendarAccountsUpsert({
                        name: name || "CalDAV",
                        caldavUrl: url,
                        username,
                        password,
                        collections: discovered,
                      })
                      .then(() => {
                        setName("");
                        setUrl("");
                        setUsername("");
                        setPassword("");
                        setDiscovered([]);
                        setStatus(t("caldavSaved"));
                        return refresh();
                      })
                      .catch((err) => setError((err as AppError).message));
                  }}
                >
                  {t("caldavAdd")}
                </Button>
              </div>
              <ul className="mt-4 space-y-2">
                {accounts.map((acc) => (
                  <li
                    key={acc.id}
                    className="flex flex-wrap items-center justify-between gap-2 rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] px-3 py-2.5 text-sm"
                  >
                    <span>
                      <span className="font-medium">{acc.name}</span>{" "}
                      <span className="text-[var(--nova-ink-muted)]">
                        {acc.username}
                      </span>
                    </span>
                    <div className="flex gap-2">
                      <Button
                        size="sm"
                        variant="secondary"
                        onClick={() => {
                          void api
                            .calendarAccountsSync(acc.id)
                            .then(([ev, tk]) => {
                              setStatus(
                                t("caldavSynced")
                                  .replace("{events}", String(ev))
                                  .replace("{tasks}", String(tk)),
                              );
                              return refresh();
                            })
                            .catch((err) =>
                              setError((err as AppError).message),
                            );
                        }}
                      >
                        {t("sync")}
                      </Button>
                      <Button
                        size="sm"
                        variant="ghost"
                        onClick={() => {
                          void api
                            .calendarAccountsDelete(acc.id)
                            .then(refresh)
                            .catch((err) =>
                              setError((err as AppError).message),
                            );
                        }}
                      >
                        {t("remove")}
                      </Button>
                    </div>
                  </li>
                ))}
              </ul>
            </section>
          </div>
        ) : null}
      </div>

      {conflictPrompt ? (
        <div className="absolute inset-0 z-30 flex items-center justify-center bg-[color-mix(in_srgb,var(--nova-ink)_35%,transparent)] p-4">
          <div
            role="dialog"
            aria-labelledby="cal-conflict-title"
            className="w-full max-w-md rounded-[var(--nova-radius-lg)] border border-[var(--nova-border)] bg-[var(--nova-surface)] p-5 shadow-[var(--nova-shadow)]"
          >
            <h2
              id="cal-conflict-title"
              className="font-[family-name:var(--nova-font-display)] text-xl"
            >
              {t("calendarConflictTitle")}
            </h2>
            <p className="mt-2 text-sm text-[var(--nova-ink-muted)]">
              {t("calendarConflictBody", {
                title:
                  events.find((ev) => ev.id === conflictPrompt.primary.id)
                    ?.title ?? "…",
              })}
            </p>
            <ul className="mt-3 max-h-40 space-y-1 overflow-y-auto text-sm">
              {conflictPrompt.push.map((item) => (
                <li key={item.id} className="text-[var(--nova-ink)]">
                  → {item.title}{" "}
                  <span className="text-[var(--nova-ink-muted)]">
                    {formatTime(item.startsAt, locale)}–
                    {formatTime(item.endsAt, locale)}
                  </span>
                </li>
              ))}
            </ul>
            <div className="mt-4 flex flex-wrap justify-end gap-2">
              <Button
                type="button"
                variant="ghost"
                onClick={() => {
                  setConflictPrompt(null);
                  void refresh();
                }}
              >
                {t("cancel")}
              </Button>
              <Button
                type="button"
                variant="secondary"
                onClick={() => {
                  const commit = conflictPrompt.primary;
                  setConflictPrompt(null);
                  void applyTimedCommit(commit, false);
                }}
              >
                {t("calendarConflictKeep")}
              </Button>
              <Button
                type="button"
                onClick={() => {
                  const commit = conflictPrompt.primary;
                  setConflictPrompt(null);
                  void applyTimedCommit(commit, true);
                }}
              >
                {t("calendarConflictPush")}
              </Button>
            </div>
          </div>
        </div>
      ) : null}

      {draft ? (
        <div className="absolute inset-0 z-20 flex justify-end bg-[color-mix(in_srgb,var(--nova-ink)_28%,transparent)]">
          <div className="nova-slide-in flex h-full w-full max-w-md flex-col border-l border-[var(--nova-border)] bg-[var(--nova-surface)] shadow-[var(--nova-shadow)]">
            <div className="flex items-center justify-between border-b border-[var(--nova-border)] px-5 py-4">
              <h2 className="font-[family-name:var(--nova-font-display)] text-xl">
                {draft.id ? t("calendarEditEvent") : t("calendarNewEvent")}
              </h2>
              <Button size="sm" variant="ghost" onClick={() => setDraft(null)}>
                ✕
              </Button>
            </div>
            <div className="flex-1 space-y-3 overflow-y-auto px-5 py-4">
              <Input
                value={draft.title}
                onChange={(e) => setDraft({ ...draft, title: e.target.value })}
                placeholder={t("calendarEventTitle")}
                autoFocus
              />
              <label className="flex items-center gap-2 text-sm">
                <input
                  type="checkbox"
                  checked={draft.allDay}
                  onChange={(e) =>
                    setDraft({ ...draft, allDay: e.target.checked })
                  }
                />
                {t("calendarAllDay")}
              </label>
              <fieldset className="grid gap-2">
                <legend className="text-xs text-[var(--nova-ink-muted)]">
                  {t("calendarStarts")}
                </legend>
                <div
                  className={
                    draft.allDay
                      ? "grid gap-2"
                      : "grid grid-cols-2 gap-2"
                  }
                >
                  <label className="grid gap-1 text-xs text-[var(--nova-ink-muted)]">
                    <span>{t("calendarDate")}</span>
                    <input
                      type="date"
                      className="w-full rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] bg-transparent px-3 py-2 text-sm text-[var(--nova-ink)]"
                      value={toLocalDate(draft.startsAt)}
                      onChange={(e) => {
                        const startsAt = withLocalDate(
                          draft.startsAt,
                          e.target.value,
                        );
                        setDraft({
                          ...draft,
                          startsAt,
                          endsAt: Math.max(draft.endsAt, startsAt + 1800),
                        });
                      }}
                    />
                  </label>
                  {draft.allDay ? null : (
                    <label className="grid gap-1 text-xs text-[var(--nova-ink-muted)]">
                      <span>{t("calendarTime")}</span>
                      <input
                        type="time"
                        step={60}
                        className="w-full rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] bg-transparent px-3 py-2 text-sm text-[var(--nova-ink)]"
                        value={toLocalTime(draft.startsAt)}
                        onChange={(e) => {
                          const startsAt = withLocalTime(
                            draft.startsAt,
                            e.target.value,
                          );
                          setDraft({
                            ...draft,
                            startsAt,
                            endsAt: Math.max(draft.endsAt, startsAt + 1800),
                          });
                        }}
                      />
                    </label>
                  )}
                </div>
              </fieldset>
              <fieldset className="grid gap-2">
                <legend className="text-xs text-[var(--nova-ink-muted)]">
                  {t("calendarEnds")}
                </legend>
                <div
                  className={
                    draft.allDay
                      ? "grid gap-2"
                      : "grid grid-cols-2 gap-2"
                  }
                >
                  <label className="grid gap-1 text-xs text-[var(--nova-ink-muted)]">
                    <span>{t("calendarDate")}</span>
                    <input
                      type="date"
                      className="w-full rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] bg-transparent px-3 py-2 text-sm text-[var(--nova-ink)]"
                      value={toLocalDate(draft.endsAt)}
                      onChange={(e) =>
                        setDraft({
                          ...draft,
                          endsAt: withLocalDate(draft.endsAt, e.target.value),
                        })
                      }
                    />
                  </label>
                  {draft.allDay ? null : (
                    <label className="grid gap-1 text-xs text-[var(--nova-ink-muted)]">
                      <span>{t("calendarTime")}</span>
                      <input
                        type="time"
                        step={60}
                        className="w-full rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] bg-transparent px-3 py-2 text-sm text-[var(--nova-ink)]"
                        value={toLocalTime(draft.endsAt)}
                        onChange={(e) =>
                          setDraft({
                            ...draft,
                            endsAt: withLocalTime(draft.endsAt, e.target.value),
                          })
                        }
                      />
                    </label>
                  )}
                </div>
              </fieldset>
              <Input
                value={draft.location}
                onChange={(e) =>
                  setDraft({ ...draft, location: e.target.value })
                }
                placeholder={t("calendarLocation")}
              />
              <textarea
                value={draft.description}
                onChange={(e) =>
                  setDraft({ ...draft, description: e.target.value })
                }
                placeholder={t("calendarNotes")}
                rows={4}
                className="w-full rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] bg-transparent px-3 py-2 text-sm"
              />
              <label className="block text-xs text-[var(--nova-ink-muted)]">
                {t("calendarReminder")}
                <select
                  className="mt-1 w-full rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] bg-transparent px-3 py-2 text-sm"
                  value={draft.reminderMinutes}
                  onChange={(e) =>
                    setDraft({
                      ...draft,
                      reminderMinutes: Number(e.target.value),
                    })
                  }
                >
                  {REMINDER_OPTIONS.map((m) => (
                    <option key={m} value={m}>
                      {m === 0
                        ? t("calendarReminderAtStart")
                        : t("calendarReminderMinutes").replace(
                            "{n}",
                            String(m),
                          )}
                    </option>
                  ))}
                </select>
              </label>
              <label className="block text-xs text-[var(--nova-ink-muted)]">
                {t("calendarManageTitle")}
                <select
                  className="mt-1 w-full rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] bg-transparent px-3 py-2 text-sm"
                  value={draft.collectionId}
                  onChange={(e) =>
                    setDraft({ ...draft, collectionId: e.target.value })
                  }
                >
                  {collections.map((c) => (
                    <option key={c.id} value={c.id}>
                      {c.displayName}
                      {c.isDefault ? ` (${t("calendarMain")})` : ""}
                    </option>
                  ))}
                </select>
              </label>
            </div>
            <div className="flex gap-2 border-t border-[var(--nova-border)] px-5 py-4">
              {draft.id ? (
                <Button
                  size="sm"
                  variant="ghost"
                  disabled={saving}
                  onClick={() => void deleteDraft()}
                >
                  {t("remove")}
                </Button>
              ) : null}
              <div className="flex-1" />
              <Button
                size="sm"
                variant="secondary"
                onClick={() => setDraft(null)}
              >
                {t("cancel")}
              </Button>
              <Button
                size="sm"
                disabled={saving || !draft.title.trim()}
                onClick={() => void saveDraft()}
              >
                {t("save")}
              </Button>
            </div>
          </div>
        </div>
      ) : null}
    </section>
  );
}

function startOfDay(ms: number): number {
  const d = new Date(ms);
  d.setHours(0, 0, 0, 0);
  return d.getTime();
}

function startOfMonth(ms: number): number {
  const d = new Date(ms);
  d.setDate(1);
  d.setHours(0, 0, 0, 0);
  return d.getTime();
}

/** Monday-first week start (ISO-style, German office default). */
function startOfWeekMonday(ms: number): number {
  const d = new Date(startOfDay(ms));
  const day = d.getDay(); // 0=Sun
  const diff = day === 0 ? -6 : 1 - day;
  d.setDate(d.getDate() + diff);
  return d.getTime();
}

function formatTime(unix: number, locale: string): string {
  return new Date(unix * 1000).toLocaleTimeString(
    locale === "de" ? "de-DE" : "en-US",
    { hour: "2-digit", minute: "2-digit" },
  );
}

function formatDateTime(unix: number, locale: string): string {
  return new Date(unix * 1000).toLocaleString(
    locale === "de" ? "de-DE" : "en-US",
    {
      weekday: "short",
      day: "numeric",
      month: "short",
      hour: "2-digit",
      minute: "2-digit",
    },
  );
}

function pad2(n: number): string {
  return String(n).padStart(2, "0");
}

function toLocalDate(unix: number): string {
  const d = new Date(unix * 1000);
  return `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())}`;
}

function toLocalTime(unix: number): string {
  const d = new Date(unix * 1000);
  return `${pad2(d.getHours())}:${pad2(d.getMinutes())}`;
}

/** Combine YYYY-MM-DD + HH:MM into unix seconds (local timezone). */
function combineLocal(date: string, time: string): number {
  const normalizedTime = time.length === 5 ? `${time}:00` : time || "00:00:00";
  const parsed = Date.parse(`${date}T${normalizedTime}`);
  return Number.isFinite(parsed)
    ? Math.floor(parsed / 1000)
    : Math.floor(Date.now() / 1000);
}

function withLocalDate(unix: number, date: string): number {
  if (!date) return unix;
  return combineLocal(date, toLocalTime(unix));
}

function withLocalTime(unix: number, time: string): number {
  if (!time) return unix;
  return combineLocal(toLocalDate(unix), time);
}
