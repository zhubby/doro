"use client";

import { History, PauseCircle, Play, Plus, RefreshCw, Trash2 } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useTranslations } from "next-intl";

import { DataTable } from "@/components/admin/data-table";
import { PageSection } from "@/components/admin/page-section";
import { Toolbar } from "@/components/admin/toolbar";
import { PageContainer } from "@/components/layout/page-container";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  createScheduledTask,
  deleteScheduledTask,
  getScheduledTaskRuns,
  getScheduledTasks,
  scheduledTaskAction,
} from "@/lib/control-plane-api";
import { formatRelativeTime } from "@/lib/datetime";
import type {
  ScheduledTask,
  ScheduledTaskKind,
  ScheduledTaskRun,
  ScheduledTaskStatus,
} from "@/types/api";
import type { ResourceColumn } from "@/types/dashboard";

type ScheduledTaskForm = {
  name: string;
  kind: ScheduledTaskKind;
  schedule: string;
  labels: string;
  script: string;
  prompt: string;
  timeoutSeconds: string;
};

const emptyForm: ScheduledTaskForm = {
  name: "",
  kind: "script",
  schedule: "0 3 * * *",
  labels: "agent",
  script: "",
  prompt: "",
  timeoutSeconds: "30",
};

function splitLabels(value: string) {
  return value
    .split(",")
    .map((label) => label.trim())
    .filter((label, index, labels) => label.length > 0 && labels.indexOf(label) === index);
}

function statusVariant(status: ScheduledTaskStatus) {
  if (status === "active") {
    return "default";
  }
  if (status === "pending_approval") {
    return "secondary";
  }
  return "outline";
}

function runStatusVariant(status: ScheduledTaskRun["status"]) {
  if (status === "succeeded") {
    return "default";
  }
  if (status === "running" || status === "skipped") {
    return "secondary";
  }
  return "destructive";
}

export function CronPage() {
  const t = useTranslations("resources.cron");
  const tColumns = useTranslations("resources.columns");
  const tActions = useTranslations("common.actions");
  const [items, setItems] = useState<ScheduledTask[]>([]);
  const [runs, setRuns] = useState<ScheduledTaskRun[]>([]);
  const [runsFor, setRunsFor] = useState<ScheduledTask | null>(null);
  const [form, setForm] = useState<ScheduledTaskForm>(emptyForm);
  const [createOpen, setCreateOpen] = useState(false);
  const [runsOpen, setRunsOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [pending, setPending] = useState<string | null>(null);

  async function load() {
    setLoading(true);
    const result = await getScheduledTasks();
    setLoading(false);
    if (result.error || !result.data) {
      setError(result.error ?? t("errors.load"));
      return;
    }
    setError(null);
    setItems(result.data.items);
  }

  useEffect(() => {
    void load();
  }, []);

  const columns = useMemo<ResourceColumn<ScheduledTask>[]>(
    () => [
      {
        key: "name",
        label: tColumns("name"),
        width: "22%",
        render: (row) => (
          <div className="min-w-0">
            <p className="truncate font-medium" title={row.name}>
              {row.name}
            </p>
            <p className="truncate text-xs text-muted-foreground">
              {row.kind === "script" ? t("kindScript") : t("kindAgent")}
            </p>
          </div>
        ),
      },
      {
        key: "schedule",
        label: tColumns("schedule"),
        width: "16%",
        render: (row) => <span className="font-mono text-xs">{row.schedule}</span>,
      },
      {
        key: "label_selector",
        label: t("labels"),
        width: "18%",
        render: (row) => (
          <div className="flex flex-wrap gap-1">
            {row.label_selector.length > 0 ? (
              row.label_selector.map((label) => (
                <Badge key={label} variant="outline">
                  {label}
                </Badge>
              ))
            ) : (
              <span className="text-muted-foreground">{t("allAgents")}</span>
            )}
          </div>
        ),
      },
      {
        key: "status",
        label: tColumns("status"),
        width: "12%",
        render: (row) => (
          <Badge variant={statusVariant(row.status)}>{t(`status.${row.status}`)}</Badge>
        ),
      },
      {
        key: "next_run_at",
        label: t("nextRun"),
        width: "14%",
        render: (row) => (
          <span title={row.next_run_at ?? ""}>
            {formatRelativeTime(row.next_run_at, { emptyText: "-" })}
          </span>
        ),
      },
      {
        key: "last_run_at",
        label: tColumns("lastRun"),
        width: "14%",
        render: (row) => (
          <div>
            <p title={row.last_run_at ?? ""}>
              {formatRelativeTime(row.last_run_at, { emptyText: "-" })}
            </p>
            {row.last_run_status ? (
              <p className="text-xs text-muted-foreground">
                {t(`runStatus.${row.last_run_status}`)}
              </p>
            ) : null}
          </div>
        ),
      },
    ],
    [t, tColumns],
  );

  async function handleCreate() {
    setPending("create");
    const result = await createScheduledTask({
      name: form.name,
      kind: form.kind,
      schedule: form.schedule,
      label_selector: splitLabels(form.labels),
      script: form.kind === "script" ? form.script : null,
      prompt: form.kind === "agent_run" ? form.prompt : null,
      timeout_seconds:
        form.kind === "script" && form.timeoutSeconds
          ? Number(form.timeoutSeconds)
          : null,
    });
    setPending(null);
    if (result.error || !result.data) {
      setError(result.error ?? t("errors.create"));
      return;
    }
    const data = result.data;
    setItems((current) => [data.item, ...current]);
    setCreateOpen(false);
    setForm(emptyForm);
    setError(null);
  }

  async function handleAction(item: ScheduledTask, action: "enable" | "disable" | "run") {
    setPending(`${item.id}:${action}`);
    const result = await scheduledTaskAction(item.id, action);
    setPending(null);
    if (result.error || !result.data) {
      setError(result.error ?? t("errors.action"));
      return;
    }
    const data = result.data;
    setItems((current) =>
      current.map((row) => (row.id === item.id ? data.item : row)),
    );
    setError(null);
  }

  async function handleDelete(item: ScheduledTask) {
    setPending(`${item.id}:delete`);
    const result = await deleteScheduledTask(item.id);
    setPending(null);
    if (result.error) {
      setError(result.error);
      return;
    }
    setItems((current) => current.filter((row) => row.id !== item.id));
    setError(null);
  }

  async function handleRuns(item: ScheduledTask) {
    setRunsFor(item);
    setRunsOpen(true);
    setPending(`${item.id}:runs`);
    const result = await getScheduledTaskRuns(item.id);
    setPending(null);
    if (result.error || !result.data) {
      setError(result.error ?? t("errors.runs"));
      setRuns([]);
      return;
    }
    setRuns(result.data.items);
  }

  return (
    <PageContainer>
      {error ? (
        <PageSection>
          <div className="rounded-md border border-destructive/40 bg-destructive/10 px-4 py-3 text-sm text-destructive">
            {error}
          </div>
        </PageSection>
      ) : null}

      <PageSection title={t("title")} description={t("description")} contentClassName="space-y-4">
        <Toolbar
          left={
            <Button onClick={() => setCreateOpen(true)}>
              <Plus className="size-4" aria-hidden="true" />
              {t("create")}
            </Button>
          }
          right={
            <Button variant="outline" size="icon" aria-label={t("refresh")} onClick={load}>
              <RefreshCw className="size-4" aria-hidden="true" />
            </Button>
          }
        />
        <DataTable
          columns={columns}
          rows={items}
          emptyText={loading ? t("loading") : t("empty")}
          actionsWidth="15rem"
          renderActions={(item) => {
            const isActive = item.status === "active";
            return (
              <>
                <Button
                  variant="outline"
                  size="sm"
                  disabled={pending === `${item.id}:run`}
                  onClick={() => void handleAction(item, "run")}
                >
                  <Play className="size-4" aria-hidden="true" />
                  {t("run")}
                </Button>
                <Button
                  variant="outline"
                  size="icon"
                  aria-label={isActive ? t("disable") : t("enable")}
                  disabled={pending === `${item.id}:${isActive ? "disable" : "enable"}`}
                  onClick={() => void handleAction(item, isActive ? "disable" : "enable")}
                >
                  {isActive ? (
                    <PauseCircle className="size-4" aria-hidden="true" />
                  ) : (
                    <Play className="size-4" aria-hidden="true" />
                  )}
                </Button>
                <Button
                  variant="outline"
                  size="icon"
                  aria-label={t("history")}
                  disabled={pending === `${item.id}:runs`}
                  onClick={() => void handleRuns(item)}
                >
                  <History className="size-4" aria-hidden="true" />
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  aria-label={tActions("delete")}
                  disabled={pending === `${item.id}:delete`}
                  onClick={() => void handleDelete(item)}
                >
                  <Trash2 className="size-4" aria-hidden="true" />
                </Button>
              </>
            );
          }}
        />
      </PageSection>

      <Dialog open={createOpen} onOpenChange={setCreateOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("create")}</DialogTitle>
            <DialogDescription>{t("createDescription")}</DialogDescription>
          </DialogHeader>
          <div className="grid gap-4">
            <label className="grid gap-2 text-sm font-medium">
              {t("fields.name")}
              <input
                className="h-10 rounded-md border bg-background px-3 text-sm"
                value={form.name}
                onChange={(event) => setForm({ ...form, name: event.target.value })}
              />
            </label>
            <div className="grid grid-cols-2 gap-3">
              <label className="grid gap-2 text-sm font-medium">
                {t("fields.kind")}
                <select
                  className="h-10 rounded-md border bg-background px-3 text-sm"
                  value={form.kind}
                  onChange={(event) =>
                    setForm({ ...form, kind: event.target.value as ScheduledTaskKind })
                  }
                >
                  <option value="script">{t("kindScript")}</option>
                  <option value="agent_run">{t("kindAgent")}</option>
                </select>
              </label>
              <label className="grid gap-2 text-sm font-medium">
                {t("fields.schedule")}
                <input
                  className="h-10 rounded-md border bg-background px-3 font-mono text-sm"
                  value={form.schedule}
                  onChange={(event) => setForm({ ...form, schedule: event.target.value })}
                />
              </label>
            </div>
            <label className="grid gap-2 text-sm font-medium">
              {t("fields.labels")}
              <input
                className="h-10 rounded-md border bg-background px-3 text-sm"
                value={form.labels}
                onChange={(event) => setForm({ ...form, labels: event.target.value })}
              />
            </label>
            {form.kind === "script" ? (
              <>
                <label className="grid gap-2 text-sm font-medium">
                  {t("fields.script")}
                  <textarea
                    className="min-h-36 rounded-md border bg-background p-3 font-mono text-sm"
                    value={form.script}
                    onChange={(event) => setForm({ ...form, script: event.target.value })}
                  />
                </label>
                <label className="grid gap-2 text-sm font-medium">
                  {t("fields.timeout")}
                  <input
                    className="h-10 rounded-md border bg-background px-3 text-sm"
                    inputMode="numeric"
                    value={form.timeoutSeconds}
                    onChange={(event) =>
                      setForm({ ...form, timeoutSeconds: event.target.value })
                    }
                  />
                </label>
              </>
            ) : (
              <label className="grid gap-2 text-sm font-medium">
                {t("fields.prompt")}
                <textarea
                  className="min-h-28 rounded-md border bg-background p-3 text-sm"
                  value={form.prompt}
                  onChange={(event) => setForm({ ...form, prompt: event.target.value })}
                />
              </label>
            )}
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setCreateOpen(false)}>
              {tActions("cancel")}
            </Button>
            <Button
              disabled={
                pending === "create" ||
                !form.name.trim() ||
                !form.schedule.trim() ||
                (form.kind === "script" ? !form.script.trim() : !form.prompt.trim())
              }
              onClick={() => void handleCreate()}
            >
              {t("create")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog open={runsOpen} onOpenChange={setRunsOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{runsFor?.name ?? t("history")}</DialogTitle>
            <DialogDescription>{t("historyDescription")}</DialogDescription>
          </DialogHeader>
          <div className="max-h-96 overflow-auto rounded-md border">
            {runs.length === 0 ? (
              <div className="p-6 text-center text-sm text-muted-foreground">
                {t("emptyRuns")}
              </div>
            ) : (
              <div className="divide-y">
                {runs.map((run) => (
                  <div key={run.id} className="grid grid-cols-[1fr_auto] gap-3 p-3 text-sm">
                    <div className="min-w-0">
                      <p className="truncate font-mono text-xs" title={run.task_id ?? run.id}>
                        {run.task_id ?? run.id}
                      </p>
                      <p className="text-xs text-muted-foreground" title={run.message ?? ""}>
                        {run.message ?? "-"}
                      </p>
                    </div>
                    <div className="text-right">
                      <Badge variant={runStatusVariant(run.status)}>
                        {t(`runStatus.${run.status}`)}
                      </Badge>
                      <p className="mt-1 text-xs text-muted-foreground" title={run.started_at}>
                        {formatRelativeTime(run.started_at)}
                      </p>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        </DialogContent>
      </Dialog>
    </PageContainer>
  );
}
