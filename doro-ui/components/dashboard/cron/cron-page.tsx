"use client";

import {
  Bot,
  CalendarClock,
  CheckCircle2,
  Clock3,
  History,
  ListChecks,
  PauseCircle,
  Play,
  Plus,
  RefreshCw,
  ScrollText,
  ShieldCheck,
  Trash2,
  type LucideIcon,
} from "lucide-react";
import { useEffect, useMemo, useState, type ReactNode } from "react";
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
import { Select } from "@/components/ui/select";
import { useToastMessage } from "@/components/ui/use-toast-message";
import {
  createScheduledTask,
  deleteScheduledTask,
  getScheduledTaskRuns,
  getScheduledTasks,
  scheduledTaskAction,
} from "@/lib/control-plane-api";
import { formatRelativeTime } from "@/lib/datetime";
import { cn } from "@/lib/utils";
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

const inputClass =
  "h-9 w-full rounded-md border bg-background px-3 text-sm outline-none ring-offset-background transition-colors placeholder:text-muted-foreground focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-60";

const textareaClass =
  "min-h-28 w-full resize-y rounded-md border bg-background px-3 py-2 text-sm outline-none ring-offset-background transition-colors placeholder:text-muted-foreground focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-60";

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

function SummaryTile({
  icon: Icon,
  label,
  value,
  tone,
}: {
  icon: LucideIcon;
  label: string;
  value: string;
  tone: "total" | "active" | "pending" | "paused";
}) {
  const toneClass = {
    total: "bg-muted text-muted-foreground",
    active: "bg-emerald-500/10 text-emerald-700 dark:text-emerald-300",
    pending: "bg-amber-500/10 text-amber-700 dark:text-amber-300",
    paused: "bg-slate-500/10 text-slate-700 dark:text-slate-300",
  }[tone];

  return (
    <div className="rounded-lg border bg-background p-3 transition-colors hover:bg-muted/30">
      <div className="flex items-center justify-between gap-3">
        <span className="text-xs font-medium text-muted-foreground">{label}</span>
        <span className={cn("flex size-7 items-center justify-center rounded-md", toneClass)}>
          <Icon className="size-4" aria-hidden="true" />
        </span>
      </div>
      <p className="mt-2 text-2xl font-semibold tracking-tight">{value}</p>
    </div>
  );
}

function FormSection({
  title,
  description,
  children,
}: {
  title: string;
  description: string;
  children: ReactNode;
}) {
  return (
    <section className="rounded-lg border bg-muted/20 p-3">
      <div className="mb-3">
        <h3 className="text-sm font-semibold">{title}</h3>
        <p className="mt-1 text-xs leading-5 text-muted-foreground">{description}</p>
      </div>
      <div className="grid gap-3 sm:grid-cols-2">{children}</div>
    </section>
  );
}

function Field({
  label,
  hint,
  children,
  className,
}: {
  label: string;
  hint?: string;
  children: ReactNode;
  className?: string;
}) {
  return (
    <label className={cn("block space-y-2 text-sm", className)}>
      <span className="block font-medium">{label}</span>
      {children}
      {hint ? (
        <span className="block text-xs leading-5 text-muted-foreground">{hint}</span>
      ) : null}
    </label>
  );
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

  useToastMessage(error, { id: "cron-error", kind: "error" });

  const stats = useMemo(
    () => ({
      total: items.length,
      active: items.filter((item) => item.status === "active").length,
      pendingApproval: items.filter((item) => item.status === "pending_approval").length,
      paused: items.filter((item) => item.status === "paused").length,
    }),
    [items],
  );

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
        width: "24%",
        render: (row) => {
          const KindIcon = row.kind === "script" ? ScrollText : Bot;

          return (
            <div className="flex min-w-0 items-start gap-2">
              <div className="mt-0.5 flex size-8 shrink-0 items-center justify-center rounded-md border bg-muted/40 text-muted-foreground">
                <KindIcon className="size-4" aria-hidden="true" />
              </div>
              <div className="min-w-0">
                <p className="truncate font-medium" title={row.name}>
                  {row.name}
                </p>
                <p className="truncate text-xs text-muted-foreground">
                  {row.kind === "script" ? t("kindScript") : t("kindAgent")}
                </p>
              </div>
            </div>
          );
        },
      },
      {
        key: "schedule",
        label: tColumns("schedule"),
        width: "16%",
        render: (row) => (
          <span
            className="inline-flex max-w-full items-center gap-1 rounded-md border bg-muted/40 px-2 py-1 font-mono text-xs"
            title={row.schedule}
          >
            <CalendarClock
              className="size-3.5 shrink-0 text-muted-foreground"
              aria-hidden="true"
            />
            <span className="truncate">{row.schedule}</span>
          </span>
        ),
      },
      {
        key: "label_selector",
        label: t("labels"),
        width: "17%",
        render: (row) => (
          <div className="flex min-w-0 flex-wrap gap-1">
            {row.label_selector.length > 0 ? (
              row.label_selector.map((label) => (
                <Badge
                  key={label}
                  variant="outline"
                  className="max-w-full bg-background font-medium"
                  title={label}
                >
                  <span className="truncate">{label}</span>
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
        width: "11%",
        render: (row) => (
          <Badge variant={statusVariant(row.status)} className="justify-center">
            {t(`status.${row.status}`)}
          </Badge>
        ),
      },
      {
        key: "next_run_at",
        label: t("nextRun"),
        width: "14%",
        render: (row) => (
          <div className="flex min-w-0 items-center gap-2" title={row.next_run_at ?? ""}>
            <Clock3 className="size-3.5 shrink-0 text-muted-foreground" aria-hidden="true" />
            <span className="truncate">
              {formatRelativeTime(row.next_run_at, { emptyText: "-" })}
            </span>
          </div>
        ),
      },
      {
        key: "last_run_at",
        label: tColumns("lastRun"),
        width: "14%",
        render: (row) => (
          <div className="min-w-0 space-y-1">
            <p className="truncate" title={row.last_run_at ?? ""}>
              {formatRelativeTime(row.last_run_at, { emptyText: "-" })}
            </p>
            {row.last_run_status ? (
              <Badge
                variant={runStatusVariant(row.last_run_status)}
                className="px-1.5 py-0 text-[11px] font-medium"
              >
                {t(`runStatus.${row.last_run_status}`)}
              </Badge>
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

  const createPending = pending === "create";
  const createDisabled =
    createPending ||
    !form.name.trim() ||
    !form.schedule.trim() ||
    (form.kind === "script" ? !form.script.trim() : !form.prompt.trim());

  return (
    <PageContainer>
      <PageSection title={t("title")} description={t("description")} contentClassName="space-y-4">
        <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
          <SummaryTile
            icon={ListChecks}
            label={t("summary.total")}
            value={loading ? "-" : String(stats.total)}
            tone="total"
          />
          <SummaryTile
            icon={CheckCircle2}
            label={t("summary.active")}
            value={loading ? "-" : String(stats.active)}
            tone="active"
          />
          <SummaryTile
            icon={Clock3}
            label={t("summary.pending")}
            value={loading ? "-" : String(stats.pendingApproval)}
            tone="pending"
          />
          <SummaryTile
            icon={PauseCircle}
            label={t("summary.paused")}
            value={loading ? "-" : String(stats.paused)}
            tone="paused"
          />
        </div>
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
          emptyText={loading ? t("loading") : t("emptyState")}
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
        <DialogContent className="max-h-[90vh] max-w-2xl overflow-y-auto">
          <DialogHeader>
            <DialogTitle>{t("create")}</DialogTitle>
            <DialogDescription>{t("createDescription")}</DialogDescription>
          </DialogHeader>

          <div className="space-y-4">
            <FormSection
              title={t("sections.identity")}
              description={t("sections.identityDescription")}
            >
              <Field
                label={t("fields.name")}
                hint={t("hints.name")}
                className="sm:col-span-2"
              >
                <input
                  className={inputClass}
                  value={form.name}
                  disabled={createPending}
                  placeholder={t("placeholders.name")}
                  onChange={(event) => setForm({ ...form, name: event.target.value })}
                />
              </Field>
              <Field label={t("fields.kind")} hint={t("hints.kind")}>
                <Select
                  value={form.kind}
                  disabled={createPending}
                  onValueChange={(value) =>
                    setForm({ ...form, kind: value as ScheduledTaskKind })
                  }
                  options={[
                    { value: "script", label: t("kindScript") },
                    { value: "agent_run", label: t("kindAgent") },
                  ]}
                />
              </Field>
              <Field label={t("fields.schedule")} hint={t("hints.schedule")}>
                <input
                  className={cn(inputClass, "font-mono")}
                  value={form.schedule}
                  disabled={createPending}
                  placeholder={t("placeholders.schedule")}
                  onChange={(event) => setForm({ ...form, schedule: event.target.value })}
                />
              </Field>
            </FormSection>

            <FormSection
              title={t("sections.scope")}
              description={t("sections.scopeDescription")}
            >
              <Field
                label={t("fields.labels")}
                hint={t("hints.labels")}
                className="sm:col-span-2"
              >
                <input
                  className={inputClass}
                  value={form.labels}
                  disabled={createPending}
                  placeholder={t("placeholders.labels")}
                  onChange={(event) => setForm({ ...form, labels: event.target.value })}
                />
              </Field>
            </FormSection>

            <FormSection
              title={t("sections.execution")}
              description={t("sections.executionDescription")}
            >
              {form.kind === "script" ? (
                <>
                  <Field
                    label={t("fields.script")}
                    hint={t("hints.script")}
                    className="sm:col-span-2"
                  >
                    <textarea
                      className={cn(textareaClass, "min-h-40 font-mono")}
                      value={form.script}
                      disabled={createPending}
                      placeholder={t("placeholders.script")}
                      onChange={(event) => setForm({ ...form, script: event.target.value })}
                    />
                  </Field>
                  <Field label={t("fields.timeout")} hint={t("hints.timeout")}>
                    <input
                      className={inputClass}
                      inputMode="numeric"
                      type="number"
                      min="1"
                      value={form.timeoutSeconds}
                      disabled={createPending}
                      placeholder={t("placeholders.timeout")}
                      onChange={(event) =>
                        setForm({ ...form, timeoutSeconds: event.target.value })
                      }
                    />
                  </Field>
                </>
              ) : (
                <Field
                  label={t("fields.prompt")}
                  hint={t("hints.prompt")}
                  className="sm:col-span-2"
                >
                  <textarea
                    className={cn(textareaClass, "min-h-32")}
                    value={form.prompt}
                    disabled={createPending}
                    placeholder={t("placeholders.prompt")}
                    onChange={(event) => setForm({ ...form, prompt: event.target.value })}
                  />
                </Field>
              )}
            </FormSection>

            <div className="flex gap-3 rounded-lg border bg-muted/30 p-3 text-sm">
              <ShieldCheck
                className="mt-0.5 size-4 shrink-0 text-muted-foreground"
                aria-hidden="true"
              />
              <div className="min-w-0">
                <p className="font-medium">{t("approvalNoteTitle")}</p>
                <p className="mt-1 text-xs leading-5 text-muted-foreground">
                  {t("approvalNoteDescription")}
                </p>
              </div>
            </div>
          </div>

          <DialogFooter className="gap-2 sm:space-x-0">
            <Button
              variant="outline"
              disabled={createPending}
              onClick={() => setCreateOpen(false)}
            >
              {tActions("cancel")}
            </Button>
            <Button disabled={createDisabled} onClick={() => void handleCreate()}>
              {createPending ? t("creating") : t("create")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog open={runsOpen} onOpenChange={setRunsOpen}>
        <DialogContent className="max-w-xl">
          <DialogHeader>
            <DialogTitle>{runsFor?.name ?? t("history")}</DialogTitle>
            <DialogDescription>{t("historyDescription")}</DialogDescription>
          </DialogHeader>
          <div className="max-h-96 overflow-auto rounded-lg border">
            {runs.length === 0 ? (
              <div className="flex min-h-32 flex-col items-center justify-center gap-2 p-6 text-center text-sm text-muted-foreground">
                <History className="size-5" aria-hidden="true" />
                {t("emptyRuns")}
              </div>
            ) : (
              <div className="divide-y">
                {runs.map((run) => (
                  <div
                    key={run.id}
                    className="grid grid-cols-[minmax(0,1fr)_auto] gap-3 p-3 text-sm transition-colors hover:bg-muted/30"
                  >
                    <div className="min-w-0">
                      <p
                        className="truncate font-mono text-xs font-medium"
                        title={run.task_id ?? run.id}
                      >
                        {run.task_id ?? run.id}
                      </p>
                      <p
                        className="mt-1 truncate text-xs text-muted-foreground"
                        title={run.message ?? ""}
                      >
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
