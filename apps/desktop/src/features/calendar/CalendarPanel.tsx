import { useCallback, useEffect, useMemo, useState } from "react";
import { Button, Input } from "@novamail/ui";

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

type ViewMode = "day" | "week";
type PanelTab = "schedule" | "inbox" | "manage";

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
  const [view, setView] = useState<ViewMode>("week");
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
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [draft, setDraft] = useState<EventDraft | null>(null);
  const [saving, setSaving] = useState(false);

  const range = useMemo(() => {
    const from = Math.floor(anchor / 1000);
    const days = view === "day" ? 1 : 7;
    const to = from + days * 86400 - 1;
    return { from, to, days };
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

  const days = useMemo(() => {
    return Array.from({ length: range.days }, (_, i) => {
      const start = anchor + i * 86400000;
      const dayStart = Math.floor(start / 1000);
      const dayEnd = dayStart + 86399;
      return {
        start,
        dayStart,
        events: events.filter(
          (ev) =>
            ev.startsAt <= dayEnd &&
            (ev.endsAt == null || ev.endsAt >= dayStart),
        ),
      };
    });
  }, [anchor, events, range.days]);

  const openNewEvent = (dayStartMs?: number) => {
    const base = dayStartMs ?? Date.now();
    const startsAt = Math.floor(base / 1000);
    const rounded = startsAt - (startsAt % 3600) + 3600;
    setDraft({
      title: "",
      startsAt: rounded,
      endsAt: rounded + 3600,
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

  const hourSlots = useMemo(() => Array.from({ length: 14 }, (_, i) => i + 7), []);

  return (
    <section className="relative flex h-full flex-col overflow-hidden">
      <div className="nova-fade-in flex flex-wrap items-end justify-between gap-3 border-b border-[var(--nova-border)] px-6 py-4">
        <div>
          <h1 className="font-[family-name:var(--nova-font-display)] text-2xl tracking-tight">
            {t("calendar")}
          </h1>
          <p className="mt-0.5 text-sm text-[var(--nova-ink-muted)]">
            {t("calendarDescription")}
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <div className="flex rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] p-0.5">
            {(
              [
                ["schedule", t("calendar")],
                ["inbox", `${t("calendarInbox")}${invites.length ? ` (${invites.length})` : ""}`],
                ["manage", t("calendarManage")],
              ] as const
            ).map(([key, label]) => (
              <button
                key={key}
                type="button"
                onClick={() => setTab(key)}
                className={
                  tab === key
                    ? "rounded-[calc(var(--nova-radius-md)-2px)] bg-[var(--nova-accent-soft)] px-3 py-1.5 text-sm font-medium text-[var(--nova-accent)]"
                    : "px-3 py-1.5 text-sm text-[var(--nova-ink-muted)] hover:text-[var(--nova-ink)]"
                }
              >
                {label}
              </button>
            ))}
          </div>
          {tab === "schedule" ? (
            <>
              <Button size="sm" variant="secondary" onClick={() => setView("day")}>
                {t("calendarDay")}
              </Button>
              <Button size="sm" variant="secondary" onClick={() => setView("week")}>
                {t("calendarWeek")}
              </Button>
              <Button
                size="sm"
                variant="ghost"
                onClick={() => setAnchor(startOfDay(Date.now()))}
              >
                {t("calendarToday")}
              </Button>
              <Button
                size="sm"
                variant="ghost"
                onClick={() => setAnchor((a) => a - range.days * 86400000)}
              >
                ←
              </Button>
              <Button
                size="sm"
                variant="ghost"
                onClick={() => setAnchor((a) => a + range.days * 86400000)}
              >
                →
              </Button>
              <Button size="sm" onClick={() => openNewEvent()}>
                + {t("calendarNewEvent")}
              </Button>
            </>
          ) : null}
        </div>
      </div>

      {(error || status) && (
        <p
          className={`px-6 py-2 text-sm ${error ? "text-[var(--nova-danger)]" : "text-[var(--nova-ink-muted)]"}`}
        >
          {error ?? status}
        </p>
      )}

      <div className="min-h-0 flex-1 overflow-y-auto px-6 py-4">
        {tab === "schedule" ? (
          <div className="nova-fade-in space-y-6">
            {view === "day" ? (
              <div className="relative rounded-[var(--nova-radius-md)] border border-[var(--nova-border)]">
                <div className="border-b border-[var(--nova-border)] px-4 py-2 text-sm font-medium">
                  {formatDayLabel(days[0]?.start ?? anchor, locale)}
                </div>
                <div className="relative">
                  {hourSlots.map((hour) => {
                    const slotStart =
                      (days[0]?.dayStart ?? Math.floor(anchor / 1000)) +
                      hour * 3600;
                    const slotEnd = slotStart + 3600;
                    const slotEvents = (days[0]?.events ?? []).filter(
                      (ev) =>
                        !ev.allDay &&
                        ev.startsAt < slotEnd &&
                        (ev.endsAt ?? ev.startsAt + 3600) > slotStart,
                    );
                    return (
                      <div
                        key={hour}
                        className="grid min-h-[52px] grid-cols-[56px_1fr] border-b border-[var(--nova-border)] last:border-0"
                      >
                        <button
                          type="button"
                          className="px-2 py-1 text-right text-xs text-[var(--nova-ink-muted)] hover:text-[var(--nova-accent)]"
                          onClick={() =>
                            openNewEvent(
                              ((days[0]?.dayStart ?? 0) + hour * 3600) * 1000,
                            )
                          }
                        >
                          {String(hour).padStart(2, "0")}:00
                        </button>
                        <div className="relative flex flex-col gap-1 px-2 py-1">
                          {slotEvents.map((ev) => (
                            <button
                              key={ev.id}
                              type="button"
                              onClick={() => openEditEvent(ev)}
                              className="rounded-md px-2 py-1 text-left text-sm text-white transition-transform hover:scale-[1.01]"
                              style={{
                                background: ev.color ?? "var(--nova-accent)",
                              }}
                            >
                              <span className="font-medium">{ev.title}</span>
                              {ev.location ? (
                                <span className="ml-2 opacity-90">
                                  · {ev.location}
                                </span>
                              ) : null}
                            </button>
                          ))}
                        </div>
                      </div>
                    );
                  })}
                  {(days[0]?.events ?? [])
                    .filter((ev) => ev.allDay)
                    .map((ev) => (
                      <button
                        key={ev.id}
                        type="button"
                        onClick={() => openEditEvent(ev)}
                        className="m-2 block w-[calc(100%-1rem)] rounded-md px-2 py-1 text-left text-sm text-white"
                        style={{ background: ev.color ?? "var(--nova-accent)" }}
                      >
                        {ev.title}
                      </button>
                    ))}
                </div>
              </div>
            ) : (
              <div
                className={`grid gap-2 ${range.days === 7 ? "md:grid-cols-7" : "grid-cols-1"}`}
              >
                {days.map((day) => (
                  <div
                    key={day.start}
                    className="min-h-[140px] rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] p-2"
                  >
                    <button
                      type="button"
                      className="mb-2 w-full text-left text-xs font-medium text-[var(--nova-ink-muted)] hover:text-[var(--nova-accent)]"
                      onClick={() => openNewEvent(day.start)}
                    >
                      {formatDayLabel(day.start, locale)}
                    </button>
                    <ul className="space-y-1">
                      {day.events.map((ev) => (
                        <li key={ev.id}>
                          <button
                            type="button"
                            onClick={() => openEditEvent(ev)}
                            className="flex w-full items-start gap-1.5 rounded-md px-1.5 py-1 text-left text-xs transition-colors hover:bg-[var(--nova-accent-soft)]"
                          >
                            <span
                              className="mt-1 h-2 w-2 shrink-0 rounded-full"
                              style={{
                                background: ev.color ?? "var(--nova-accent)",
                              }}
                            />
                            <span>
                              {!ev.allDay ? (
                                <span className="text-[var(--nova-ink-muted)]">
                                  {formatTime(ev.startsAt, locale)}{" "}
                                </span>
                              ) : null}
                              {ev.title}
                            </span>
                          </button>
                        </li>
                      ))}
                    </ul>
                  </div>
                ))}
              </div>
            )}

            <section>
              <h2 className="mb-2 text-xs font-medium uppercase tracking-wide text-[var(--nova-ink-muted)]">
                {t("tasks")}
              </h2>
              <div className="mb-2 flex gap-2">
                <Input
                  value={newTaskTitle}
                  onChange={(e) => setNewTaskTitle(e.target.value)}
                  placeholder={t("taskTitle")}
                />
                <Button
                  size="sm"
                  onClick={() => {
                    if (!newTaskTitle.trim()) return;
                    void api
                      .calendarTasksUpsert({
                        title: newTaskTitle.trim(),
                        dueAt: Math.floor(Date.now() / 1000) + 86400,
                      })
                      .then(() => {
                        setNewTaskTitle("");
                        return refresh();
                      })
                      .catch((err) => setError((err as AppError).message));
                  }}
                >
                  {t("add")}
                </Button>
              </div>
              <ul className="space-y-1">
                {tasks.map((task) => (
                  <li
                    key={task.id}
                    className="flex items-center gap-2 text-sm"
                  >
                    <input
                      type="checkbox"
                      checked={task.completed}
                      onChange={() => {
                        void api
                          .calendarTasksUpsert({
                            id: task.id,
                            title: task.title,
                            dueAt: task.dueAt,
                            completed: !task.completed,
                            notes: task.notes,
                          })
                          .then(refresh)
                          .catch((err) => setError((err as AppError).message));
                      }}
                    />
                    <span
                      className={
                        task.completed
                          ? "text-[var(--nova-ink-muted)] line-through"
                          : undefined
                      }
                    >
                      {task.title}
                    </span>
                    <Button
                      size="sm"
                      variant="ghost"
                      onClick={() => {
                        void api
                          .calendarTasksDelete(task.id)
                          .then(refresh)
                          .catch((err) => setError((err as AppError).message));
                      }}
                    >
                      {t("remove")}
                    </Button>
                  </li>
                ))}
              </ul>
            </section>
          </div>
        ) : null}

        {tab === "inbox" ? (
          <div className="nova-slide-in mx-auto max-w-xl space-y-3">
            <p className="text-sm text-[var(--nova-ink-muted)]">
              {t("calendarInboxHint")}
            </p>
            {invites.length === 0 ? (
              <p className="text-sm text-[var(--nova-ink-muted)]">
                {t("calendarInboxEmpty")}
              </p>
            ) : (
              invites.map((inv) => (
                <article
                  key={inv.id}
                  className="rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] p-4"
                >
                  <h3 className="font-medium">{inv.title}</h3>
                  <p className="mt-1 text-sm text-[var(--nova-ink-muted)]">
                    {formatDateTime(inv.startsAt, locale)}
                    {inv.location ? ` · ${inv.location}` : ""}
                  </p>
                  {inv.organizer ? (
                    <p className="text-sm text-[var(--nova-ink-muted)]">
                      {inv.organizer}
                    </p>
                  ) : null}
                  <div className="mt-3 flex flex-wrap gap-2">
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

        {tab === "manage" ? (
          <div className="nova-fade-in mx-auto max-w-2xl space-y-8">
            <section>
              <h2 className="mb-3 text-xs font-medium uppercase tracking-wide text-[var(--nova-ink-muted)]">
                {t("calendarManage")}
              </h2>
              <ul className="space-y-2">
                {collections.map((col) => (
                  <li
                    key={col.id}
                    className="flex flex-wrap items-center gap-3 rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] px-3 py-2"
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
                    <label className="flex items-center gap-1 text-xs">
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
                  </li>
                ))}
              </ul>
              <div className="mt-3 flex flex-wrap gap-2">
                {COLOR_PALETTE.map((c) => (
                  <button
                    key={c}
                    type="button"
                    title={c}
                    className="h-6 w-6 rounded-full border border-[var(--nova-border)]"
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
              <h2 className="mb-3 text-xs font-medium uppercase tracking-wide text-[var(--nova-ink-muted)]">
                {t("caldavAccounts")}
              </h2>
              <div className="grid gap-2 sm:grid-cols-2">
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
              <div className="mt-2 flex flex-wrap gap-2">
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
              <ul className="mt-3 space-y-2">
                {accounts.map((acc) => (
                  <li
                    key={acc.id}
                    className="flex flex-wrap items-center justify-between gap-2 rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] px-3 py-2 text-sm"
                  >
                    <span>
                      {acc.name}{" "}
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

      {draft ? (
        <div className="absolute inset-0 z-20 flex justify-end bg-[color-mix(in_srgb,var(--nova-ink)_25%,transparent)]">
          <div className="nova-slide-in flex h-full w-full max-w-md flex-col border-l border-[var(--nova-border)] bg-[var(--nova-surface)] shadow-xl">
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
                onChange={(e) =>
                  setDraft({ ...draft, title: e.target.value })
                }
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
              <label className="block text-xs text-[var(--nova-ink-muted)]">
                {t("calendarStarts")}
                <input
                  type="datetime-local"
                  className="mt-1 w-full rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] bg-transparent px-3 py-2 text-sm"
                  value={toLocalInput(draft.startsAt)}
                  onChange={(e) => {
                    const startsAt = fromLocalInput(e.target.value);
                    setDraft({
                      ...draft,
                      startsAt,
                      endsAt: Math.max(draft.endsAt, startsAt + 1800),
                    });
                  }}
                />
              </label>
              <label className="block text-xs text-[var(--nova-ink-muted)]">
                {t("calendarEnds")}
                <input
                  type="datetime-local"
                  className="mt-1 w-full rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] bg-transparent px-3 py-2 text-sm"
                  value={toLocalInput(draft.endsAt)}
                  onChange={(e) =>
                    setDraft({
                      ...draft,
                      endsAt: fromLocalInput(e.target.value),
                    })
                  }
                />
              </label>
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
                {t("calendar")}
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

function formatDayLabel(ms: number, locale: string): string {
  return new Date(ms).toLocaleDateString(locale === "de" ? "de-DE" : "en-US", {
    weekday: "short",
    day: "numeric",
    month: "short",
  });
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

function toLocalInput(unix: number): string {
  const d = new Date(unix * 1000);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

function fromLocalInput(value: string): number {
  const t = Date.parse(value);
  return Number.isFinite(t) ? Math.floor(t / 1000) : Math.floor(Date.now() / 1000);
}
