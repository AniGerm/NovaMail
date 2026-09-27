import { useCallback, useEffect, useMemo, useState } from "react";
import { Button, Input } from "@novamail/ui";

import { api } from "@/shared/api/client";
import type {
  AppError,
  CalendarAccountDto,
  CalendarEventDto,
  CalendarTaskDto,
} from "@/shared/api/types";
import { useT } from "@/shared/i18n/useT";
import { formatMessageDate } from "@/shared/lib/format";
import { useUiStore } from "@/shared/store/uiStore";

type ViewMode = "day" | "week";

export function CalendarPanel() {
  const t = useT();
  const locale = useUiStore((s) => s.locale);
  const [view, setView] = useState<ViewMode>("week");
  const [anchor, setAnchor] = useState(() => startOfDay(Date.now()));
  const [events, setEvents] = useState<CalendarEventDto[]>([]);
  const [tasks, setTasks] = useState<CalendarTaskDto[]>([]);
  const [accounts, setAccounts] = useState<CalendarAccountDto[]>([]);
  const [name, setName] = useState("");
  const [url, setUrl] = useState("");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [newEventTitle, setNewEventTitle] = useState("");
  const [newTaskTitle, setNewTaskTitle] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<string | null>(null);

  const range = useMemo(() => {
    const from = Math.floor(anchor / 1000);
    const days = view === "day" ? 1 : 7;
    const to = from + days * 86400 - 1;
    return { from, to, days };
  }, [anchor, view]);

  const refresh = useCallback(async () => {
    try {
      const [e, ta, ac] = await Promise.all([
        api.calendarEventsList(range.from, range.to),
        api.calendarTasksList(true),
        api.calendarAccountsList(),
      ]);
      setEvents(e);
      setTasks(ta);
      setAccounts(ac);
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
        events: events.filter(
          (ev) =>
            ev.startsAt <= dayEnd &&
            (ev.endsAt == null || ev.endsAt >= dayStart),
        ),
      };
    });
  }, [anchor, events, range.days]);

  return (
    <section className="flex h-full flex-col overflow-y-auto px-6 py-5">
      <div className="mb-4 flex flex-wrap items-end justify-between gap-3">
        <div>
          <h2 className="font-[family-name:var(--nova-font-display)] text-2xl">
            {t("calendar")}
          </h2>
          <p className="text-sm text-[var(--nova-ink-muted)]">
            {t("calendarDescription")}
          </p>
        </div>
        <div className="flex flex-wrap gap-2">
          <Button
            type="button"
            size="sm"
            variant={view === "day" ? "primary" : "secondary"}
            onClick={() => setView("day")}
          >
            {t("calendarDay")}
          </Button>
          <Button
            type="button"
            size="sm"
            variant={view === "week" ? "primary" : "secondary"}
            onClick={() => setView("week")}
          >
            {t("calendarWeek")}
          </Button>
          <Button
            type="button"
            size="sm"
            variant="secondary"
            onClick={() => setAnchor((a) => a - range.days * 86400000)}
          >
            ←
          </Button>
          <Button
            type="button"
            size="sm"
            variant="secondary"
            onClick={() => setAnchor(startOfDay(Date.now()))}
          >
            {t("calendarToday")}
          </Button>
          <Button
            type="button"
            size="sm"
            variant="secondary"
            onClick={() => setAnchor((a) => a + range.days * 86400000)}
          >
            →
          </Button>
        </div>
      </div>

      {error ? (
        <p className="mb-3 text-sm text-[var(--nova-danger)]" role="alert">
          {error}
        </p>
      ) : null}
      {status ? (
        <p className="mb-3 text-sm text-[var(--nova-accent)]" role="status">
          {status}
        </p>
      ) : null}

      <div
        className={
          view === "week"
            ? "mb-8 grid gap-3 md:grid-cols-7"
            : "mb-8 grid gap-3"
        }
      >
        {days.map((day) => (
          <div
            key={day.start}
            className="min-h-[140px] rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] px-3 py-2"
          >
            <p className="mb-2 text-xs font-semibold uppercase tracking-wide text-[var(--nova-ink-muted)]">
              {new Date(day.start).toLocaleDateString(locale, {
                weekday: "short",
                day: "numeric",
                month: "short",
              })}
            </p>
            {day.events.length === 0 ? (
              <p className="text-xs text-[var(--nova-ink-muted)]">—</p>
            ) : (
              <ul className="space-y-1">
                {day.events.map((ev) => (
                  <li key={ev.id} className="text-sm">
                    <span className="font-medium">{ev.title}</span>
                    {!ev.allDay ? (
                      <span className="ml-1 text-xs text-[var(--nova-ink-muted)]">
                        {formatMessageDate(ev.startsAt, locale)}
                      </span>
                    ) : null}
                  </li>
                ))}
              </ul>
            )}
          </div>
        ))}
      </div>

      <div className="mb-6 grid gap-3 md:grid-cols-2">
        <div>
          <h3 className="mb-2 text-sm font-semibold uppercase tracking-wide text-[var(--nova-ink-muted)]">
            {t("calendarNewEvent")}
          </h3>
          <div className="flex gap-2">
            <Input
              value={newEventTitle}
              onChange={(e) => setNewEventTitle(e.target.value)}
              placeholder={t("calendarEventTitle")}
            />
            <Button
              type="button"
              disabled={!newEventTitle.trim()}
              onClick={() => {
                const startsAt = Math.floor(Date.now() / 1000) + 3600;
                void api
                  .calendarEventsUpsert({
                    title: newEventTitle.trim(),
                    startsAt,
                    endsAt: startsAt + 3600,
                    calendarAccountId: accounts[0]?.id ?? null,
                  })
                  .then(() => {
                    setNewEventTitle("");
                    return refresh();
                  })
                  .catch((err) => setError((err as AppError).message));
              }}
            >
              {t("add")}
            </Button>
          </div>
        </div>
        <div>
          <h3 className="mb-2 text-sm font-semibold uppercase tracking-wide text-[var(--nova-ink-muted)]">
            {t("tasks")}
          </h3>
          <div className="mb-2 flex gap-2">
            <Input
              value={newTaskTitle}
              onChange={(e) => setNewTaskTitle(e.target.value)}
              placeholder={t("taskTitle")}
            />
            <Button
              type="button"
              disabled={!newTaskTitle.trim()}
              onClick={() => {
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
                className="flex items-center justify-between gap-2 text-sm"
              >
                <label className="flex min-w-0 items-center gap-2">
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
                          calendarAccountId: task.calendarAccountId,
                          sourceMessageId: task.sourceMessageId,
                        })
                        .then(refresh)
                        .catch((err) => setError((err as AppError).message));
                    }}
                  />
                  <span
                    className={
                      task.completed
                        ? "truncate text-[var(--nova-ink-muted)] line-through"
                        : "truncate"
                    }
                  >
                    {task.title}
                  </span>
                </label>
                <Button
                  type="button"
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
        </div>
      </div>

      <h3 className="mb-2 text-sm font-semibold uppercase tracking-wide text-[var(--nova-ink-muted)]">
        {t("caldavAccounts")}
      </h3>
      <div className="mb-3 grid gap-2 md:grid-cols-2">
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
      <div className="mb-4 flex flex-wrap gap-2">
        <Button
          type="button"
          variant="secondary"
          disabled={!url.trim() || !username.trim()}
          onClick={() => {
            void api
              .calendarDiscover({
                caldavUrl: url.trim(),
                username: username.trim(),
                password: password || "",
              })
              .then((cols) => {
                if (cols.length === 0) {
                  setStatus(t("caldavCollections") + ": 0");
                  return;
                }
                const first = cols[0]!;
                setUrl(first.href);
                if (!name.trim()) setName(first.displayName);
                setStatus(
                  `${t("caldavCollections")}: ${cols
                    .map((c) => c.displayName)
                    .join(", ")}`,
                );
              })
              .catch((err) => setError((err as AppError).message));
          }}
        >
          {t("caldavDiscover")}
        </Button>
        <Button
          type="button"
          disabled={!name.trim() || !url.trim()}
          onClick={() => {
            void api
              .calendarAccountsUpsert({
                name: name.trim(),
                caldavUrl: url.trim(),
                username: username.trim(),
                password: password || null,
              })
              .then(() => {
                setName("");
                setUrl("");
                setUsername("");
                setPassword("");
                setStatus(t("caldavSaved"));
                return refresh();
              })
              .catch((err) => setError((err as AppError).message));
          }}
        >
          {t("caldavAdd")}
        </Button>
      </div>
      <p className="mb-4 text-xs text-[var(--nova-ink-muted)]">
        {t("caldavWriteback")}
      </p>
      <ul className="space-y-2">
        {accounts.map((account) => (
          <li
            key={account.id}
            className="flex flex-wrap items-center justify-between gap-2 rounded-[var(--nova-radius-md)] border border-[var(--nova-border)] px-3 py-2"
          >
            <div className="min-w-0">
              <p className="truncate text-sm font-medium">{account.name}</p>
              <p className="truncate text-xs text-[var(--nova-ink-muted)]">
                {account.caldavUrl}
              </p>
            </div>
            <div className="flex gap-2">
              <Button
                type="button"
                size="sm"
                variant="secondary"
                onClick={() => {
                  void api
                    .calendarAccountsSync(account.id)
                    .then(([ev, ta]) => {
                      setStatus(
                        t("caldavSynced", { events: ev, tasks: ta }),
                      );
                      return refresh();
                    })
                    .catch((err) => setError((err as AppError).message));
                }}
              >
                {t("sync")}
              </Button>
              <Button
                type="button"
                size="sm"
                variant="ghost"
                onClick={() => {
                  void api
                    .calendarAccountsDelete(account.id)
                    .then(refresh)
                    .catch((err) => setError((err as AppError).message));
                }}
              >
                {t("remove")}
              </Button>
            </div>
          </li>
        ))}
      </ul>
    </section>
  );
}

function startOfDay(ms: number): number {
  const d = new Date(ms);
  d.setHours(0, 0, 0, 0);
  return d.getTime();
}
