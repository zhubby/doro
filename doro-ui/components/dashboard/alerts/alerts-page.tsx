"use client";

import { BellRing, Pencil, Plus, RefreshCw, Trash2 } from "lucide-react";
import { useEffect, useMemo, useState } from "react";

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
import { Switch } from "@/components/ui/switch";
import { useToastMessage } from "@/components/ui/use-toast-message";
import {
  createAlertRule,
  deleteAlertRule,
  getAlertIncidents,
  getAlertRules,
  getHosts,
  updateAlertRule,
} from "@/lib/control-plane-api";
import { formatRelativeTime } from "@/lib/datetime";
import type {
  AlertIncident,
  AlertMetricSource,
  AlertOperator,
  AlertRule,
  AlertSeverity,
  CreateAlertRuleRequest,
  Host,
  UpdateAlertRuleRequest,
} from "@/types/api";
import type { ResourceColumn } from "@/types/dashboard";

type RuleFormState = {
  id?: string;
  name: string;
  description: string;
  severity: AlertSeverity;
  metricSource: AlertMetricSource;
  coreMetricKey: string;
  extraMetricKey: string;
  operator: AlertOperator;
  threshold: string;
  hostId: string;
  enabled: boolean;
  forSeconds: string;
  cooldownSeconds: string;
};

const emptyRuleForm: RuleFormState = {
  name: "",
  description: "",
  severity: "warning",
  metricSource: "core",
  coreMetricKey: "cpu_percent",
  extraMetricKey: "/gpus/0/utilization_percent",
  operator: "greater_than",
  threshold: "90",
  hostId: "all",
  enabled: true,
  forSeconds: "60",
  cooldownSeconds: "600",
};

const inputClass =
  "h-9 w-full rounded-md border bg-background px-3 text-sm outline-none ring-offset-background transition-colors placeholder:text-muted-foreground focus-visible:ring-2 focus-visible:ring-ring";

const textareaClass =
  "min-h-20 w-full rounded-md border bg-background px-3 py-2 text-sm outline-none ring-offset-background transition-colors placeholder:text-muted-foreground focus-visible:ring-2 focus-visible:ring-ring";

const coreMetricOptions = [
  { value: "cpu_percent", label: "CPU 使用率" },
  { value: "memory_percent", label: "内存使用率" },
  { value: "disk_percent", label: "磁盘使用率" },
  { value: "load_average", label: "Load Average" },
];

const severityOptions = [
  { value: "info", label: "提示" },
  { value: "warning", label: "警告" },
  { value: "critical", label: "严重" },
];

const operatorOptions = [
  { value: "greater_than", label: ">" },
  { value: "greater_than_or_equal", label: ">=" },
  { value: "less_than", label: "<" },
  { value: "less_than_or_equal", label: "<=" },
  { value: "equal", label: "=" },
  { value: "not_equal", label: "!=" },
];

const metricSourceOptions = [
  { value: "core", label: "核心指标" },
  { value: "extra", label: "extra JSON 路径" },
];

function severityBadge(severity: AlertSeverity) {
  if (severity === "critical") {
    return <Badge variant="destructive">严重</Badge>;
  }
  if (severity === "warning") {
    return <Badge>警告</Badge>;
  }
  return <Badge variant="secondary">提示</Badge>;
}

function incidentStatusBadge(status: AlertIncident["status"]) {
  return status === "firing" ? (
    <Badge variant="destructive">触发中</Badge>
  ) : (
    <Badge variant="secondary">已恢复</Badge>
  );
}

function operatorLabel(operator: AlertOperator) {
  return operatorOptions.find((option) => option.value === operator)?.label ?? operator;
}

function metricLabel(rule: AlertRule) {
  if (rule.metric.source === "core") {
    return (
      coreMetricOptions.find((option) => option.value === rule.metric.key)?.label ??
      rule.metric.key
    );
  }
  return `extra${rule.metric.key}`;
}

function incidentMetricLabel(incident: AlertIncident) {
  if (incident.metric.source === "core") {
    return (
      coreMetricOptions.find((option) => option.value === incident.metric.key)?.label ??
      incident.metric.key
    );
  }
  return `extra${incident.metric.key}`;
}

function normalizeSeconds(value: string, fallback: number) {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isFinite(parsed) || parsed < 0) {
    return fallback;
  }
  return parsed;
}

function formToRequest(form: RuleFormState): CreateAlertRuleRequest {
  return {
    name: form.name.trim(),
    description: form.description.trim() || null,
    severity: form.severity,
    metric: {
      source: form.metricSource,
      key:
        form.metricSource === "core"
          ? form.coreMetricKey
          : form.extraMetricKey.trim(),
    },
    operator: form.operator,
    threshold: Number.parseFloat(form.threshold),
    host_id: form.hostId === "all" ? null : form.hostId,
    enabled: form.enabled,
    for_seconds: normalizeSeconds(form.forSeconds, 60),
    cooldown_seconds: normalizeSeconds(form.cooldownSeconds, 600),
  };
}

function formToUpdateRequest(form: RuleFormState): UpdateAlertRuleRequest {
  const request = formToRequest(form);
  return {
    name: request.name,
    description: request.description,
    severity: request.severity,
    metric: request.metric,
    operator: request.operator,
    threshold: request.threshold,
    host_id: request.host_id,
    enabled: request.enabled,
    for_seconds: request.for_seconds,
    cooldown_seconds: request.cooldown_seconds,
  };
}

function ruleToForm(rule: AlertRule): RuleFormState {
  return {
    id: rule.id,
    name: rule.name,
    description: rule.description ?? "",
    severity: rule.severity,
    metricSource: rule.metric.source,
    coreMetricKey: rule.metric.source === "core" ? rule.metric.key : "cpu_percent",
    extraMetricKey:
      rule.metric.source === "extra" ? rule.metric.key : "/gpus/0/utilization_percent",
    operator: rule.operator,
    threshold: String(rule.threshold),
    hostId: rule.host_id ?? "all",
    enabled: rule.enabled,
    forSeconds: String(rule.for_seconds),
    cooldownSeconds: String(rule.cooldown_seconds),
  };
}

function hostLabel(hosts: Host[], hostId: string | null) {
  if (!hostId) {
    return "全部主机";
  }
  const host = hosts.find((item) => item.id === hostId);
  return host?.display_name || host?.hostname || hostId;
}

function ruleColumns(hosts: Host[]): ResourceColumn<AlertRule>[] {
  return [
    {
      key: "name",
      label: "规则",
      width: "25%",
      render: (rule) => (
        <div className="min-w-0">
          <p className="truncate font-medium" title={rule.name}>
            {rule.name}
          </p>
          <p className="truncate text-xs text-muted-foreground" title={rule.description ?? ""}>
            {rule.description || rule.id}
          </p>
        </div>
      ),
    },
    {
      key: "severity",
      label: "级别",
      width: "6rem",
      render: (rule) => severityBadge(rule.severity),
    },
    {
      key: "metric",
      label: "条件",
      width: "28%",
      render: (rule) => (
        <div className="min-w-0 text-xs">
          <p className="truncate font-medium" title={metricLabel(rule)}>
            {metricLabel(rule)} {operatorLabel(rule.operator)} {rule.threshold}
          </p>
          <p className="truncate text-muted-foreground">
            持续 {rule.for_seconds}s，冷却 {rule.cooldown_seconds}s
          </p>
        </div>
      ),
    },
    {
      key: "host_id",
      label: "范围",
      width: "15%",
      render: (rule) => (
        <span className="block truncate" title={hostLabel(hosts, rule.host_id)}>
          {hostLabel(hosts, rule.host_id)}
        </span>
      ),
    },
    {
      key: "enabled",
      label: "状态",
      width: "7rem",
      render: (rule) =>
        rule.enabled ? <Badge>启用</Badge> : <Badge variant="outline">停用</Badge>,
    },
    {
      key: "updated_at",
      label: "更新",
      width: "8rem",
      render: (rule) => (
        <span title={rule.updated_at}>{formatRelativeTime(rule.updated_at)}</span>
      ),
    },
  ];
}

const incidentColumns: ResourceColumn<AlertIncident>[] = [
  {
    key: "rule_name",
    label: "事件",
    width: "28%",
    render: (incident) => (
      <div className="min-w-0">
        <p className="truncate font-medium" title={incident.rule_name}>
          {incident.rule_name}
        </p>
        <p className="truncate text-xs text-muted-foreground" title={incident.id}>
          {incident.id}
        </p>
      </div>
    ),
  },
  {
    key: "status",
    label: "状态",
    width: "7rem",
    render: (incident) => incidentStatusBadge(incident.status),
  },
  {
    key: "metric",
    label: "指标",
    width: "24%",
    render: (incident) => (
      <div className="min-w-0 text-xs">
        <p className="truncate font-medium" title={incidentMetricLabel(incident)}>
          {incidentMetricLabel(incident)} {operatorLabel(incident.operator)} {incident.threshold}
        </p>
        <p className="truncate text-muted-foreground">
          当前值 {Number(incident.observed_value).toFixed(2)}
        </p>
      </div>
    ),
  },
  {
    key: "severity",
    label: "级别",
    width: "6rem",
    render: (incident) => severityBadge(incident.severity),
  },
  {
    key: "triggered_at",
    label: "触发",
    width: "8rem",
    render: (incident) => (
      <span title={incident.triggered_at}>{formatRelativeTime(incident.triggered_at)}</span>
    ),
  },
  {
    key: "resolved_at",
    label: "恢复",
    width: "8rem",
    render: (incident) => (
      <span title={incident.resolved_at ?? ""}>
        {incident.resolved_at ? formatRelativeTime(incident.resolved_at) : "-"}
      </span>
    ),
  },
  {
    key: "notification_count",
    label: "通知",
    width: "5rem",
    render: (incident) => String(incident.notification_count),
  },
];

export function AlertsPage() {
  const [rules, setRules] = useState<AlertRule[]>([]);
  const [incidents, setIncidents] = useState<AlertIncident[]>([]);
  const [hosts, setHosts] = useState<Host[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [dialogOpen, setDialogOpen] = useState(false);
  const [form, setForm] = useState<RuleFormState>(emptyRuleForm);
  const [savePending, setSavePending] = useState(false);
  const [actionPending, setActionPending] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);

  const hostOptions = useMemo(
    () => [
      { value: "all", label: "全部主机" },
      ...hosts.map((host) => ({
        value: host.id,
        label: host.display_name || host.hostname,
      })),
    ],
    [hosts],
  );
  const activeIncidents = incidents.filter((incident) => incident.status === "firing");
  const criticalIncidents = activeIncidents.filter(
    (incident) => incident.severity === "critical",
  );

  useToastMessage(error, {
    id: "alerts-api-error",
    kind: "error",
    prefix: "控制平面暂不可用：",
  });
  useToastMessage(actionError, {
    id: "alerts-action-error",
    kind: "error",
    prefix: "告警操作失败：",
  });

  async function load() {
    setIsRefreshing(true);
    const [rulesResult, incidentsResult, hostsResult] = await Promise.all([
      getAlertRules(),
      getAlertIncidents(100),
      getHosts(),
    ]);
    setRules(rulesResult.data?.items ?? []);
    setIncidents(incidentsResult.data?.items ?? []);
    setHosts(hostsResult.data?.items ?? []);
    setError(rulesResult.error ?? incidentsResult.error ?? hostsResult.error);
    setIsRefreshing(false);
  }

  useEffect(() => {
    let cancelled = false;
    let refreshTimer: ReturnType<typeof setTimeout> | null = null;

    async function refresh() {
      if (!cancelled) {
        await load();
      }
      if (!cancelled) {
        refreshTimer = setTimeout(refresh, 10_000);
      }
    }

    refresh();

    return () => {
      cancelled = true;
      if (refreshTimer) {
        clearTimeout(refreshTimer);
      }
    };
  }, []);

  function openCreateDialog() {
    setForm(emptyRuleForm);
    setDialogOpen(true);
  }

  function openEditDialog(rule: AlertRule) {
    setForm(ruleToForm(rule));
    setDialogOpen(true);
  }

  async function handleSave() {
    setSavePending(true);
    setActionError(null);
    const request = formToRequest(form);
    if (!request.name || !Number.isFinite(request.threshold)) {
      setActionError("请填写规则名称和有效阈值");
      setSavePending(false);
      return;
    }
    const result = form.id
      ? await updateAlertRule(form.id, formToUpdateRequest(form))
      : await createAlertRule(request);
    setSavePending(false);
    if (!result.data) {
      setActionError(result.error ?? "保存失败");
      return;
    }
    const savedRule = result.data.item;
    setRules((current) => {
      if (!form.id) {
        return [savedRule, ...current];
      }
      return current.map((rule) =>
        rule.id === savedRule.id ? savedRule : rule,
      );
    });
    setDialogOpen(false);
  }

  async function handleToggle(rule: AlertRule, enabled: boolean) {
    setActionPending(`${rule.id}:toggle`);
    setActionError(null);
    const result = await updateAlertRule(rule.id, {
      name: rule.name,
      description: rule.description,
      severity: rule.severity,
      metric: rule.metric,
      operator: rule.operator,
      threshold: rule.threshold,
      host_id: rule.host_id,
      enabled,
      for_seconds: rule.for_seconds,
      cooldown_seconds: rule.cooldown_seconds,
    });
    setActionPending(null);
    if (!result.data) {
      setActionError(result.error ?? "状态更新失败");
      return;
    }
    const savedRule = result.data.item;
    setRules((current) =>
      current.map((item) => (item.id === rule.id ? savedRule : item)),
    );
  }

  async function handleDelete(rule: AlertRule) {
    setActionPending(`${rule.id}:delete`);
    setActionError(null);
    const result = await deleteAlertRule(rule.id);
    setActionPending(null);
    if (result.error) {
      setActionError(result.error);
      return;
    }
    setRules((current) => current.filter((item) => item.id !== rule.id));
  }

  return (
    <PageContainer>
      <PageSection contentClassName="grid gap-3 md:grid-cols-3">
        <SummaryTile label="启用规则" value={String(rules.filter((rule) => rule.enabled).length)} />
        <SummaryTile label="触发中" value={String(activeIncidents.length)} />
        <SummaryTile label="严重告警" value={String(criticalIncidents.length)} />
      </PageSection>

      <PageSection
        title="告警规则"
        description="根据 Agent 上报的 metrics 配置阈值，满足条件后创建告警事件并发送邮件通知。"
        contentClassName="space-y-4"
      >
        <Toolbar
          left={
            <Button onClick={openCreateDialog}>
              <Plus className="size-4" aria-hidden="true" />
              新建规则
            </Button>
          }
          right={
            <Button variant="outline" size="icon" aria-label="刷新" onClick={load}>
              <RefreshCw
                className={isRefreshing ? "size-4 animate-spin" : "size-4"}
                aria-hidden="true"
              />
            </Button>
          }
        />
        <DataTable
          columns={ruleColumns(hosts)}
          rows={rules}
          emptyText="暂无告警规则"
          actionsWidth="10rem"
          renderActions={(rule) => (
            <>
              <Switch
                checked={rule.enabled}
                disabled={actionPending === `${rule.id}:toggle`}
                aria-label={rule.enabled ? "停用规则" : "启用规则"}
                onCheckedChange={(checked) => handleToggle(rule, checked)}
              />
              <Button variant="outline" size="icon" aria-label="编辑" onClick={() => openEditDialog(rule)}>
                <Pencil className="size-4" aria-hidden="true" />
              </Button>
              <Button
                variant="outline"
                size="icon"
                aria-label="删除"
                disabled={actionPending === `${rule.id}:delete`}
                onClick={() => handleDelete(rule)}
              >
                <Trash2 className="size-4" aria-hidden="true" />
              </Button>
            </>
          )}
        />
      </PageSection>

      <PageSection
        title="告警事件"
        description="记录规则触发、恢复和邮件通知次数。"
        contentClassName="space-y-4"
      >
        <DataTable
          columns={incidentColumns}
          rows={incidents}
          actions={[]}
          emptyText="暂无告警事件"
        />
      </PageSection>

      <Dialog open={dialogOpen} onOpenChange={setDialogOpen}>
        <DialogContent className="max-w-2xl">
          <DialogHeader>
            <DialogTitle>{form.id ? "编辑告警规则" : "新建告警规则"}</DialogTitle>
            <DialogDescription>
              支持核心 metrics 字段，也支持从 extra JSON 中按 JSON Pointer 读取数值。
            </DialogDescription>
          </DialogHeader>
          <div className="grid gap-4 md:grid-cols-2">
            <Field label="名称">
              <input
                value={form.name}
                disabled={savePending}
                onChange={(event) => setForm({ ...form, name: event.target.value })}
                className={inputClass}
                placeholder="CPU 高使用率"
              />
            </Field>
            <Field label="级别">
              <Select
                value={form.severity}
                disabled={savePending}
                onValueChange={(value) =>
                  setForm({ ...form, severity: value as AlertSeverity })
                }
                options={severityOptions}
              />
            </Field>
            <Field label="指标来源">
              <Select
                value={form.metricSource}
                disabled={savePending}
                onValueChange={(value) =>
                  setForm({ ...form, metricSource: value as AlertMetricSource })
                }
                options={metricSourceOptions}
              />
            </Field>
            <Field label="指标">
              {form.metricSource === "core" ? (
                <Select
                  value={form.coreMetricKey}
                  disabled={savePending}
                  onValueChange={(value) => setForm({ ...form, coreMetricKey: value })}
                  options={coreMetricOptions}
                />
              ) : (
                <input
                  value={form.extraMetricKey}
                  disabled={savePending}
                  onChange={(event) =>
                    setForm({ ...form, extraMetricKey: event.target.value })
                  }
                  className={inputClass}
                  placeholder="/gpus/0/utilization_percent"
                />
              )}
            </Field>
            <Field label="条件">
              <Select
                value={form.operator}
                disabled={savePending}
                onValueChange={(value) =>
                  setForm({ ...form, operator: value as AlertOperator })
                }
                options={operatorOptions}
              />
            </Field>
            <Field label="阈值">
              <input
                value={form.threshold}
                disabled={savePending}
                type="number"
                step="0.01"
                onChange={(event) => setForm({ ...form, threshold: event.target.value })}
                className={inputClass}
              />
            </Field>
            <Field label="主机范围">
              <Select
                value={form.hostId}
                disabled={savePending}
                onValueChange={(value) => setForm({ ...form, hostId: value })}
                options={hostOptions}
              />
            </Field>
            <Field label="状态">
              <div className="flex h-9 items-center gap-3 rounded-md border px-3">
                <Switch
                  checked={form.enabled}
                  disabled={savePending}
                  onCheckedChange={(checked) => setForm({ ...form, enabled: checked })}
                />
                <span className="text-sm">{form.enabled ? "启用" : "停用"}</span>
              </div>
            </Field>
            <Field label="持续时间">
              <input
                value={form.forSeconds}
                disabled={savePending}
                type="number"
                min="0"
                onChange={(event) =>
                  setForm({ ...form, forSeconds: event.target.value })
                }
                className={inputClass}
              />
            </Field>
            <Field label="冷却时间">
              <input
                value={form.cooldownSeconds}
                disabled={savePending}
                type="number"
                min="0"
                onChange={(event) =>
                  setForm({ ...form, cooldownSeconds: event.target.value })
                }
                className={inputClass}
              />
            </Field>
            <Field label="说明" className="md:col-span-2">
              <textarea
                value={form.description}
                disabled={savePending}
                onChange={(event) =>
                  setForm({ ...form, description: event.target.value })
                }
                className={textareaClass}
                placeholder="用于值班人员识别规则用途"
              />
            </Field>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setDialogOpen(false)} disabled={savePending}>
              取消
            </Button>
            <Button onClick={handleSave} disabled={savePending}>
              {savePending ? "保存中" : "保存"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </PageContainer>
  );
}

function SummaryTile({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-lg border p-4">
      <div className="flex items-center gap-2 text-sm text-muted-foreground">
        <BellRing className="size-4" aria-hidden="true" />
        {label}
      </div>
      <p className="mt-2 text-2xl font-semibold tracking-tight">{value}</p>
    </div>
  );
}

function Field({
  label,
  children,
  className,
}: {
  label: string;
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <label className={className}>
      <span className="mb-2 block text-sm font-medium">{label}</span>
      {children}
    </label>
  );
}
