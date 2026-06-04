"use client";

import {
  Cable,
  Check,
  ChevronLeft,
  ChevronRight,
  FilePenLine,
  Filter,
  Link2,
  Play,
  Plus,
  RefreshCw,
  RotateCw,
  Search,
  Square,
  Trash2,
  Unlink2,
} from "lucide-react";
import {
  type FormEvent,
  type ReactNode,
  useCallback,
  useEffect,
  useMemo,
  useState,
} from "react";

import { DataTable, ResourceStatusBadge, TruncatedText } from "@/components/admin/data-table";
import { PageSection } from "@/components/admin/page-section";
import { Toolbar } from "@/components/admin/toolbar";
import { PageContainer } from "@/components/layout/page-container";
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
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuLabel,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Select } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useToastMessage } from "@/components/ui/use-toast-message";
import {
  createDockerComposeProject,
  createDockerContainer,
  createDockerNetwork,
  createDockerVolume,
  dockerComposeAction,
  dockerContainerAction,
  dockerNetworkAction,
  dockerNetworkContainerAction,
  getDockerComposeProjects,
  getDockerContainers,
  getDockerImages,
  getDockerNetworks,
  getDockerVolumes,
  getHosts,
  pullDockerImage,
  readDockerComposeProject,
  removeDockerImage,
  removeDockerVolume,
  updateDockerComposeProject,
} from "@/lib/control-plane-api";
import type {
  DockerActionResponse,
  DockerComposeProject,
  DockerContainerRestartPolicyName,
  DockerContainerSummary,
  DockerImageSummary,
  DockerNetworkSummary,
  DockerVolumeSummary,
  Host,
} from "@/types/api";
import type { ResourceColumn, ResourceStatus } from "@/types/dashboard";

type DockerKind = "containers" | "images" | "networks" | "volumes" | "compose";

type DockerPageProps = {
  kind: DockerKind;
};

type DockerRow = {
  id: string;
  hostId: string;
  hostLabel: string;
  name: string;
  secondary: string;
  status: ResourceStatus;
  statusLabel: string;
  detailA: string;
  detailB: string;
  detailC: string;
  raw:
    | DockerContainerSummary
    | DockerImageSummary
    | DockerNetworkSummary
    | DockerVolumeSummary
    | DockerComposeProject;
};

type DialogKind =
  | "container-create"
  | "image-pull"
  | "image-remove"
  | "network-create"
  | "network-connect"
  | "network-disconnect"
  | "volume-create"
  | "compose-edit";

type ActiveDialog = {
  kind: DialogKind;
  row?: DockerRow;
} | null;

type DockerFormState = {
  hostId: string;
  name: string;
  image: string;
  requiresApproval: boolean;
  reference: string;
  tag: string;
  platform: string;
  hostname: string;
  domainname: string;
  user: string;
  workingDir: string;
  entrypoint: string;
  command: string;
  env: string;
  labels: string;
  networkMode: string;
  networkName: string;
  aliases: string;
  ipv4Address: string;
  macAddress: string;
  ports: string;
  dns: string;
  dnsSearch: string;
  extraHosts: string;
  binds: string;
  volumes: string;
  tmpfs: string;
  shmSize: string;
  restartPolicy: string;
  restartMaxRetries: string;
  autoRemove: boolean;
  privileged: boolean;
  init: boolean;
  tty: boolean;
  openStdin: boolean;
  readOnlyRootfs: boolean;
  capAdd: string;
  capDrop: string;
  devices: string;
  memory: string;
  memorySwap: string;
  cpus: string;
  cpuShares: string;
  cpusetCpus: string;
  pidsLimit: string;
  healthcheckMode: string;
  healthcheckCommand: string;
  healthcheckInterval: string;
  healthcheckTimeout: string;
  healthcheckRetries: string;
  healthcheckStartPeriod: string;
  healthcheckStartInterval: string;
  logDriver: string;
  logOptions: string;
  driver: string;
  internal: boolean;
  attachable: boolean;
  driverOpts: string;
  force: boolean;
  noprune: boolean;
  container: string;
  composeYaml: string;
  envFile: string;
  reason: string;
};

const defaultComposeYaml = "services:\n  app:\n    image: nginx:1.27\n";

const emptyForm: DockerFormState = {
  hostId: "",
  name: "",
  image: "",
  requiresApproval: false,
  reference: "",
  tag: "",
  platform: "",
  hostname: "",
  domainname: "",
  user: "",
  workingDir: "",
  entrypoint: "",
  command: "",
  env: "",
  labels: "",
  networkMode: "bridge",
  networkName: "",
  aliases: "",
  ipv4Address: "",
  macAddress: "",
  ports: "",
  dns: "",
  dnsSearch: "",
  extraHosts: "",
  binds: "",
  volumes: "",
  tmpfs: "",
  shmSize: "",
  restartPolicy: "default",
  restartMaxRetries: "",
  autoRemove: false,
  privileged: false,
  init: false,
  tty: false,
  openStdin: false,
  readOnlyRootfs: false,
  capAdd: "",
  capDrop: "",
  devices: "",
  memory: "",
  memorySwap: "",
  cpus: "",
  cpuShares: "",
  cpusetCpus: "",
  pidsLimit: "",
  healthcheckMode: "inherit",
  healthcheckCommand: "",
  healthcheckInterval: "",
  healthcheckTimeout: "",
  healthcheckRetries: "",
  healthcheckStartPeriod: "",
  healthcheckStartInterval: "",
  logDriver: "",
  logOptions: "",
  driver: "",
  internal: false,
  attachable: false,
  driverOpts: "",
  force: true,
  noprune: false,
  container: "",
  composeYaml: defaultComposeYaml,
  envFile: "",
  reason: "",
};

type ContainerCreateStep = "basic" | "network" | "storage" | "features";

const containerCreateSteps: Array<{ value: ContainerCreateStep; label: string }> = [
  { value: "basic", label: "基本信息" },
  { value: "network", label: "网络" },
  { value: "storage", label: "存储" },
  { value: "features", label: "特性" },
];

const kindMeta: Record<
  DockerKind,
  { title: string; description: string; createLabel: string }
> = {
  containers: {
    title: "容器",
    description: "实时查询 Docker 容器，创建可直接执行并保留审计，也可按需提交审批。",
    createLabel: "创建容器",
  },
  images: {
    title: "镜像",
    description: "实时查询 Docker 镜像，拉取和移除镜像会进入高风险审批链路。",
    createLabel: "拉取镜像",
  },
  networks: {
    title: "网络",
    description: "实时查询 Docker 网络，并管理网络创建和容器连接。",
    createLabel: "创建网络",
  },
  volumes: {
    title: "存储卷",
    description: "实时查询 Docker 存储卷，创建和删除操作由 Agent 执行。",
    createLabel: "创建存储卷",
  },
  compose: {
    title: "Compose",
    description: "管理 Agent 受控目录下的 Compose 项目和部署动作。",
    createLabel: "新建项目",
  },
};

const columns: ResourceColumn<DockerRow>[] = [
  {
    key: "name",
    label: "名称",
    width: "28%",
    render: (row) => (
      <div className="min-w-0">
        <p className="truncate font-medium" title={row.name}>
          {row.name}
        </p>
        <p className="truncate text-xs text-muted-foreground" title={row.secondary}>
          {row.secondary}
        </p>
      </div>
    ),
  },
  {
    key: "hostLabel",
    label: "Agent",
    width: "12rem",
    render: (row) => <TruncatedText value={row.hostLabel} />,
  },
  {
    key: "status",
    label: "状态",
    width: "7rem",
    render: (row) => <ResourceStatusBadge status={row.status} />,
  },
  {
    key: "detailA",
    label: "详情",
    width: "18%",
    render: (row) => <TruncatedText value={row.detailA} />,
  },
  {
    key: "detailB",
    label: "资源",
    width: "14%",
    render: (row) => <TruncatedText value={row.detailB} />,
  },
  {
    key: "detailC",
    label: "附加信息",
    width: "16%",
    render: (row) => <TruncatedText value={row.detailC} />,
  },
];

export function DockerPage({ kind }: DockerPageProps) {
  const [hosts, setHosts] = useState<Host[]>([]);
  const [rows, setRows] = useState<DockerRow[]>([]);
  const [query, setQuery] = useState("");
  const [hostFilter, setHostFilter] = useState("all");
  const [loading, setLoading] = useState(true);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [apiError, setApiError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [activeDialog, setActiveDialog] = useState<ActiveDialog>(null);
  const [form, setForm] = useState<DockerFormState>(emptyForm);
  const meta = kindMeta[kind];

  const dockerHosts = useMemo(
    () =>
      hosts.filter(
        (host) =>
          host.status === "online" &&
          host.capabilities.some(
            (capability) => capability.name === "containers_manage",
          ),
      ),
    [hosts],
  );

  const hostNames = useMemo(
    () => new Map(hosts.map((host) => [host.id, hostLabel(host)])),
    [hosts],
  );

  const loadData = useCallback(async () => {
    setLoading(true);
    const hostsResult = await getHosts();
    if (!hostsResult.data) {
      setApiError(hostsResult.error);
      setLoading(false);
      return;
    }

    const hostItems = hostsResult.data.items;
    setHosts(hostItems);
    const selectedHostId = hostFilter === "all" ? undefined : hostFilter;
    const result = await loadDockerRows(kind, selectedHostId);
    if (result.data) {
      setRows(toRows(kind, result.data.items, new Map(hostItems.map((host) => [host.id, hostLabel(host)]))));
      setApiError(null);
    } else {
      setApiError(result.error);
    }
    setLoading(false);
  }, [hostFilter, kind]);

  useEffect(() => {
    void loadData();
  }, [loadData]);

  useToastMessage(apiError, {
    id: `docker-${kind}-api-error`,
    kind: "error",
    prefix: "Docker 管理暂不可用：",
  });
  useToastMessage(notice, { id: `docker-${kind}-notice`, kind: "success" });

  const filteredRows = useMemo(() => {
    const normalizedQuery = query.trim().toLowerCase();
    return rows.filter((row) => {
      const matchesQuery =
        !normalizedQuery ||
        row.name.toLowerCase().includes(normalizedQuery) ||
        row.secondary.toLowerCase().includes(normalizedQuery) ||
        row.detailA.toLowerCase().includes(normalizedQuery);
      return matchesQuery;
    });
  }, [query, rows]);

  const openDialog = async (dialogKind: DialogKind, row?: DockerRow) => {
    setApiError(null);
    const base = {
      ...emptyForm,
      hostId: row?.hostId ?? (hostFilter !== "all" ? hostFilter : dockerHosts[0]?.id ?? ""),
      reason: defaultReason(dialogKind, row),
    };

    if (dialogKind === "image-remove" && row) {
      setForm({
        ...base,
        reference: row.name,
        force: true,
        noprune: false,
      });
    } else if (
      (dialogKind === "network-connect" || dialogKind === "network-disconnect") &&
      row
    ) {
      setForm({ ...base, name: row.name, force: dialogKind === "network-disconnect" });
    } else if (dialogKind === "compose-edit" && row) {
      setForm({ ...base, name: row.name, composeYaml: "正在读取 compose.yaml...", envFile: "" });
      setActiveDialog({ kind: dialogKind, row });
      const result = await readDockerComposeProject(row.name, row.hostId);
      if (result.data) {
        setForm({
          ...base,
          name: result.data.item.name,
          composeYaml: result.data.item.compose_yaml ?? defaultComposeYaml,
          envFile: result.data.item.env_file ?? "",
        });
      } else {
        setApiError(result.error);
      }
      return;
    } else if (dialogKind === "compose-edit") {
      setForm({ ...base, composeYaml: defaultComposeYaml });
    } else {
      setForm(base);
    }

    setActiveDialog({ kind: dialogKind, row });
  };

  const submitDialog = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!activeDialog) {
      return;
    }
    setBusyId(activeDialog.row?.id ?? activeDialog.kind);
    setNotice(null);
    const result = await runDialogSubmit(activeDialog, form);
    const message =
      activeDialog.kind === "container-create" && !form.requiresApproval
        ? "已创建 Docker 容器任务，Agent 正在执行该操作。"
        : "已创建 Docker 审批任务，批准后 Agent 会执行该操作。";
    handleActionResult(result, message);
    if (result.data) {
      setActiveDialog(null);
      await loadData();
    }
    setBusyId(null);
  };

  const runContainerAction = async (
    row: DockerRow,
    action: "start" | "stop" | "restart" | "delete",
  ) => {
    if (action === "delete" && !window.confirm(`删除容器 ${row.name}？`)) {
      return;
    }
    setBusyId(row.id);
    const result = await dockerContainerAction(row.name, action, {
      host_id: row.hostId,
      reason: `operator requested docker container ${action}`,
    });
    handleActionResult(result);
    await loadData();
    setBusyId(null);
  };

  const runNetworkRemove = async (row: DockerRow) => {
    if (!window.confirm(`删除网络 ${row.name}？`)) {
      return;
    }
    setBusyId(row.id);
    const result = await dockerNetworkAction(row.name, "remove", {
      host_id: row.hostId,
      reason: "operator requested docker network remove",
    });
    handleActionResult(result);
    await loadData();
    setBusyId(null);
  };

  const runVolumeRemove = async (row: DockerRow) => {
    if (!window.confirm(`删除存储卷 ${row.name}？`)) {
      return;
    }
    setBusyId(row.id);
    const result = await removeDockerVolume(row.name, {
      host_id: row.hostId,
      reason: "operator requested docker volume remove",
    });
    handleActionResult(result);
    await loadData();
    setBusyId(null);
  };

  const runCompose = async (
    row: DockerRow,
    action: "up" | "down" | "restart" | "pull" | "delete",
  ) => {
    if (action === "delete" && !window.confirm(`删除 Compose 项目 ${row.name}？`)) {
      return;
    }
    setBusyId(row.id);
    const result = await dockerComposeAction(row.name, action, {
      host_id: row.hostId,
      reason: `operator requested docker compose ${action}`,
    });
    handleActionResult(result);
    await loadData();
    setBusyId(null);
  };

  function handleActionResult(
    result: { data: DockerActionResponse | null; error: string | null },
    message = "已创建 Docker 审批任务，批准后 Agent 会执行该操作。",
  ) {
    if (result.data) {
      setNotice(`${message} 任务：${shortId(result.data.task.id)}`);
      setApiError(null);
    } else {
      setApiError(result.error);
    }
  }

  return (
    <PageContainer>
      <PageSection title={meta.title} description={meta.description} contentClassName="space-y-4">
        <Toolbar
          left={
            <Button onClick={() => openDialog(defaultCreateDialog(kind))}>
              <Plus className="size-4" aria-hidden="true" />
              {meta.createLabel}
            </Button>
          }
          right={
            <div className="flex w-full flex-col gap-2 sm:w-auto sm:flex-row">
              <label className="relative min-w-0 sm:w-72">
                <Search
                  className="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground"
                  aria-hidden="true"
                />
                <span className="sr-only">搜索 Docker 资源</span>
                <input
                  value={query}
                  onChange={(event) => setQuery(event.target.value)}
                  placeholder="搜索名称、ID 或镜像"
                  className="h-9 w-full rounded-md border bg-background pl-9 pr-3 text-sm outline-none ring-offset-background placeholder:text-muted-foreground focus-visible:ring-2 focus-visible:ring-ring"
                />
              </label>
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <Button variant="outline" className="justify-start">
                    <Filter className="size-4" aria-hidden="true" />
                    {hostFilter === "all" ? "全部 Agent" : hostNames.get(hostFilter) ?? "Agent"}
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="end">
                  <DropdownMenuLabel>Agent 筛选</DropdownMenuLabel>
                  <DropdownMenuSeparator />
                  <DropdownMenuRadioGroup value={hostFilter} onValueChange={setHostFilter}>
                    <DropdownMenuRadioItem value="all">全部 Agent</DropdownMenuRadioItem>
                    {dockerHosts.map((host) => (
                      <DropdownMenuRadioItem key={host.id} value={host.id}>
                        {hostLabel(host)}
                      </DropdownMenuRadioItem>
                    ))}
                  </DropdownMenuRadioGroup>
                </DropdownMenuContent>
              </DropdownMenu>
              <Button
                variant="outline"
                size="icon"
                aria-label="刷新"
                disabled={loading}
                onClick={loadData}
              >
                <RefreshCw
                  className={`size-4 ${loading ? "animate-spin" : ""}`}
                  aria-hidden="true"
                />
              </Button>
            </div>
          }
        />
        <DataTable
          columns={columns}
          rows={filteredRows}
          actionsWidth={kind === "compose" ? "23rem" : "19rem"}
          emptyText={loading ? "正在加载 Docker 数据..." : "暂无 Docker 数据"}
          renderActions={(row) =>
            renderActions({
              kind,
              row,
              busy: busyId === row.id,
              onContainerAction: runContainerAction,
              onImageRemove: () => openDialog("image-remove", row),
              onNetworkRemove: runNetworkRemove,
              onNetworkConnect: () => openDialog("network-connect", row),
              onNetworkDisconnect: () => openDialog("network-disconnect", row),
              onVolumeRemove: runVolumeRemove,
              onComposeEdit: () => openDialog("compose-edit", row),
              onComposeAction: runCompose,
            })
          }
        />
      </PageSection>

      <DockerDialog
        open={Boolean(activeDialog)}
        dialog={activeDialog}
        form={form}
        hosts={dockerHosts}
        onOpenChange={(open) => {
          if (!open) {
            setActiveDialog(null);
          }
        }}
        onFormChange={setForm}
        onSubmit={submitDialog}
      />
    </PageContainer>
  );
}

function renderActions({
  kind,
  row,
  busy,
  onContainerAction,
  onImageRemove,
  onNetworkRemove,
  onNetworkConnect,
  onNetworkDisconnect,
  onVolumeRemove,
  onComposeEdit,
  onComposeAction,
}: {
  kind: DockerKind;
  row: DockerRow;
  busy: boolean;
  onContainerAction: (
    row: DockerRow,
    action: "start" | "stop" | "restart" | "delete",
  ) => void;
  onImageRemove: () => void;
  onNetworkRemove: (row: DockerRow) => void;
  onNetworkConnect: () => void;
  onNetworkDisconnect: () => void;
  onVolumeRemove: (row: DockerRow) => void;
  onComposeEdit: () => void;
  onComposeAction: (
    row: DockerRow,
    action: "up" | "down" | "restart" | "pull" | "delete",
  ) => void;
}) {
  if (kind === "containers") {
    return (
      <>
        {row.status === "running" ? (
          <Button
            variant="outline"
            size="sm"
            disabled={busy}
            onClick={() => onContainerAction(row, "stop")}
          >
            <Square className="size-4" aria-hidden="true" />
            停止
          </Button>
        ) : (
          <Button
            variant="outline"
            size="sm"
            disabled={busy}
            onClick={() => onContainerAction(row, "start")}
          >
            <Play className="size-4" aria-hidden="true" />
            启动
          </Button>
        )}
        <Button
          variant="outline"
          size="sm"
          disabled={busy}
          onClick={() => onContainerAction(row, "restart")}
        >
          <RotateCw className="size-4" aria-hidden="true" />
          重启
        </Button>
        <Button
          variant="outline"
          size="sm"
          disabled={busy}
          onClick={() => onContainerAction(row, "delete")}
        >
          <Trash2 className="size-4" aria-hidden="true" />
          删除
        </Button>
      </>
    );
  }

  if (kind === "images") {
    return (
      <Button variant="outline" size="sm" disabled={busy} onClick={onImageRemove}>
        <Trash2 className="size-4" aria-hidden="true" />
        移除
      </Button>
    );
  }

  if (kind === "networks") {
    return (
      <>
        <Button variant="outline" size="sm" disabled={busy} onClick={onNetworkConnect}>
          <Link2 className="size-4" aria-hidden="true" />
          连接
        </Button>
        <Button variant="outline" size="sm" disabled={busy} onClick={onNetworkDisconnect}>
          <Unlink2 className="size-4" aria-hidden="true" />
          断开
        </Button>
        <Button
          variant="outline"
          size="sm"
          disabled={busy || row.name === "bridge" || row.name === "host" || row.name === "none"}
          onClick={() => onNetworkRemove(row)}
        >
          <Trash2 className="size-4" aria-hidden="true" />
          删除
        </Button>
      </>
    );
  }

  if (kind === "volumes") {
    return (
      <Button variant="outline" size="sm" disabled={busy} onClick={() => onVolumeRemove(row)}>
        <Trash2 className="size-4" aria-hidden="true" />
        删除
      </Button>
    );
  }

  return (
    <>
      <Button variant="outline" size="sm" disabled={busy} onClick={onComposeEdit}>
        <FilePenLine className="size-4" aria-hidden="true" />
        编辑
      </Button>
      <Button variant="outline" size="sm" disabled={busy} onClick={() => onComposeAction(row, "up")}>
        <Play className="size-4" aria-hidden="true" />
        Up
      </Button>
      <Button variant="outline" size="sm" disabled={busy} onClick={() => onComposeAction(row, "down")}>
        <Square className="size-4" aria-hidden="true" />
        Down
      </Button>
      <Button
        variant="outline"
        size="sm"
        disabled={busy}
        onClick={() => onComposeAction(row, "restart")}
      >
        <RotateCw className="size-4" aria-hidden="true" />
        重启
      </Button>
      <Button
        variant="outline"
        size="sm"
        disabled={busy}
        onClick={() => onComposeAction(row, "pull")}
      >
        <Cable className="size-4" aria-hidden="true" />
        Pull
      </Button>
      <Button
        variant="outline"
        size="sm"
        disabled={busy}
        onClick={() => onComposeAction(row, "delete")}
      >
        <Trash2 className="size-4" aria-hidden="true" />
        删除
      </Button>
    </>
  );
}

function DockerDialog({
  open,
  dialog,
  form,
  hosts,
  onOpenChange,
  onFormChange,
  onSubmit,
}: {
  open: boolean;
  dialog: ActiveDialog;
  form: DockerFormState;
  hosts: Host[];
  onOpenChange: (open: boolean) => void;
  onFormChange: (form: DockerFormState) => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
}) {
  const kind = dialog?.kind;
  const title = dialogTitle(kind);
  const description = dialogDescription(kind);
  const isContainerCreate = kind === "container-create";
  const [containerStep, setContainerStep] = useState<ContainerCreateStep>("basic");
  const currentStepIndex = containerCreateSteps.findIndex(
    (step) => step.value === containerStep,
  );

  useEffect(() => {
    if (open && isContainerCreate) {
      setContainerStep("basic");
    }
  }, [open, isContainerCreate]);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        className={
          isContainerCreate
            ? "max-h-[90vh] max-w-5xl overflow-hidden"
            : kind === "compose-edit"
              ? "max-w-4xl"
              : "max-w-2xl"
        }
      >
        <form
          onSubmit={onSubmit}
          className={
            isContainerCreate
              ? "flex max-h-[calc(90vh-3rem)] min-h-0 flex-col gap-5"
              : "space-y-5"
          }
        >
          <DialogHeader>
            <DialogTitle>{title}</DialogTitle>
            <DialogDescription>{description}</DialogDescription>
          </DialogHeader>

          {kind ? isContainerCreate ? (
            <div className="min-h-0 flex-1 overflow-y-auto pr-1">
              <ContainerCreateFields
                form={form}
                hosts={hosts}
                step={containerStep}
                onStepChange={setContainerStep}
                onFormChange={onFormChange}
              />
            </div>
          ) : (
            <DialogFields
              kind={kind}
              form={form}
              hosts={hosts}
              existingRow={dialog?.row}
              onFormChange={onFormChange}
            />
          ) : null}

          {!isContainerCreate ? (
            <Field label="审批原因">
            <textarea
              value={form.reason}
              onChange={(event) => onFormChange({ ...form, reason: event.target.value })}
              className="min-h-20 w-full rounded-md border bg-background px-3 py-2 text-sm outline-none ring-offset-background focus-visible:ring-2 focus-visible:ring-ring"
            />
            </Field>
          ) : null}

          {isContainerCreate ? (
            <ContainerCreateFooter
              form={form}
              stepIndex={Math.max(currentStepIndex, 0)}
              hosts={hosts}
              onOpenChange={onOpenChange}
              onStepChange={setContainerStep}
            />
          ) : (
            <DialogFooter>
              <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
                取消
              </Button>
              <Button type="submit" disabled={!form.hostId || hosts.length === 0}>
                提交审批
              </Button>
            </DialogFooter>
          )}
        </form>
      </DialogContent>
    </Dialog>
  );
}

function DialogFields({
  kind,
  form,
  hosts,
  existingRow,
  onFormChange,
}: {
  kind: DialogKind;
  form: DockerFormState;
  hosts: Host[];
  existingRow?: DockerRow;
  onFormChange: (form: DockerFormState) => void;
}) {
  const hostSelector = (
    <Field label="目标 Agent">
      <Select
        required
        value={form.hostId}
        disabled={Boolean(existingRow) || hosts.length === 0}
        onValueChange={(value) => onFormChange({ ...form, hostId: value })}
        options={
          hosts.length === 0
            ? [{ value: "", label: "暂无在线 Docker Agent" }]
            : hosts.map((host) => ({ value: host.id, label: hostLabel(host) }))
        }
      />
    </Field>
  );

  if (kind === "image-pull") {
    return (
      <div className="grid gap-4 sm:grid-cols-2">
        {hostSelector}
        <TextField label="镜像引用" value={form.reference} onChange={(reference) => onFormChange({ ...form, reference })} required placeholder="postgres" />
        <TextField label="Tag" value={form.tag} onChange={(tag) => onFormChange({ ...form, tag })} placeholder="16" />
        <TextField label="平台" value={form.platform} onChange={(platform) => onFormChange({ ...form, platform })} placeholder="linux/amd64" />
      </div>
    );
  }

  if (kind === "image-remove") {
    return (
      <>
        <div className="grid gap-4 sm:grid-cols-2">
          {hostSelector}
          <TextField label="镜像引用" value={form.reference} onChange={(reference) => onFormChange({ ...form, reference })} required />
        </div>
        <div className="grid gap-3 sm:grid-cols-2">
          <CheckboxField label="强制移除" checked={form.force} onChange={(force) => onFormChange({ ...form, force })} />
          <CheckboxField label="不清理父层" checked={form.noprune} onChange={(noprune) => onFormChange({ ...form, noprune })} />
        </div>
      </>
    );
  }

  if (kind === "network-create") {
    return (
      <>
        <div className="grid gap-4 sm:grid-cols-2">
          {hostSelector}
          <TextField label="网络名称" value={form.name} onChange={(name) => onFormChange({ ...form, name })} required />
          <TextField label="驱动" value={form.driver} onChange={(driver) => onFormChange({ ...form, driver })} placeholder="bridge" />
        </div>
        <div className="grid gap-3 sm:grid-cols-2">
          <CheckboxField label="Internal" checked={form.internal} onChange={(internal) => onFormChange({ ...form, internal })} />
          <CheckboxField label="Attachable" checked={form.attachable} onChange={(attachable) => onFormChange({ ...form, attachable })} />
        </div>
        <Field label="标签">
          <textarea
            value={form.labels}
            onChange={(event) => onFormChange({ ...form, labels: event.target.value })}
            placeholder="key=value，每行一个"
            className="min-h-20 w-full rounded-md border bg-background px-3 py-2 text-sm outline-none ring-offset-background focus-visible:ring-2 focus-visible:ring-ring"
          />
        </Field>
      </>
    );
  }

  if (kind === "network-connect" || kind === "network-disconnect") {
    return (
      <>
        <div className="grid gap-4 sm:grid-cols-2">
          {hostSelector}
          <TextField label="网络" value={form.name} onChange={(name) => onFormChange({ ...form, name })} required />
          <TextField label="容器 ID 或名称" value={form.container} onChange={(container) => onFormChange({ ...form, container })} required />
        </div>
        <CheckboxField label="Force" checked={form.force} onChange={(force) => onFormChange({ ...form, force })} />
      </>
    );
  }

  if (kind === "volume-create") {
    return (
      <>
        <div className="grid gap-4 sm:grid-cols-2">
          {hostSelector}
          <TextField label="存储卷名称" value={form.name} onChange={(name) => onFormChange({ ...form, name })} required />
          <TextField label="驱动" value={form.driver} onChange={(driver) => onFormChange({ ...form, driver })} placeholder="local" />
        </div>
        <div className="grid gap-4 sm:grid-cols-2">
          <Field label="Driver opts">
            <textarea
              value={form.driverOpts}
              onChange={(event) => onFormChange({ ...form, driverOpts: event.target.value })}
              placeholder="key=value，每行一个"
              className="min-h-20 w-full rounded-md border bg-background px-3 py-2 text-sm outline-none ring-offset-background focus-visible:ring-2 focus-visible:ring-ring"
            />
          </Field>
          <Field label="标签">
            <textarea
              value={form.labels}
              onChange={(event) => onFormChange({ ...form, labels: event.target.value })}
              placeholder="key=value，每行一个"
              className="min-h-20 w-full rounded-md border bg-background px-3 py-2 text-sm outline-none ring-offset-background focus-visible:ring-2 focus-visible:ring-ring"
            />
          </Field>
        </div>
      </>
    );
  }

  return (
    <>
      <div className="grid gap-4 sm:grid-cols-2">
        {hostSelector}
        <TextField
          label="项目名称"
          value={form.name}
          onChange={(name) => onFormChange({ ...form, name })}
          required
          disabled={Boolean(existingRow)}
          placeholder="media-stack"
        />
      </div>
      <Field label="compose.yaml">
        <textarea
          value={form.composeYaml}
          onChange={(event) => onFormChange({ ...form, composeYaml: event.target.value })}
          className="min-h-80 w-full rounded-md border bg-background px-3 py-2 font-mono text-xs outline-none ring-offset-background focus-visible:ring-2 focus-visible:ring-ring"
          spellCheck={false}
        />
      </Field>
      <Field label=".env">
        <textarea
          value={form.envFile}
          onChange={(event) => onFormChange({ ...form, envFile: event.target.value })}
          className="min-h-28 w-full rounded-md border bg-background px-3 py-2 font-mono text-xs outline-none ring-offset-background focus-visible:ring-2 focus-visible:ring-ring"
          spellCheck={false}
        />
      </Field>
    </>
  );
}

function ContainerCreateFields({
  form,
  hosts,
  step,
  onStepChange,
  onFormChange,
}: {
  form: DockerFormState;
  hosts: Host[];
  step: ContainerCreateStep;
  onStepChange: (step: ContainerCreateStep) => void;
  onFormChange: (form: DockerFormState) => void;
}) {
  const hostSelector = (
    <Field label="目标 Agent">
      <Select
        required
        value={form.hostId}
        disabled={hosts.length === 0}
        onValueChange={(value) => onFormChange({ ...form, hostId: value })}
        options={
          hosts.length === 0
            ? [{ value: "", label: "暂无在线 Docker Agent" }]
            : hosts.map((host) => ({ value: host.id, label: hostLabel(host) }))
        }
      />
    </Field>
  );

  return (
    <div className="space-y-5">
      <Tabs value={step} onValueChange={(value) => onStepChange(value as ContainerCreateStep)}>
        <TabsList className="grid h-auto w-full grid-cols-2 gap-1 sm:grid-cols-4">
          {containerCreateSteps.map((item) => (
            <TabsTrigger key={item.value} value={item.value} className="px-2">
              {item.label}
            </TabsTrigger>
          ))}
        </TabsList>
      </Tabs>

      {step === "basic" ? (
        <ContainerBasicFields
          form={form}
          hostSelector={hostSelector}
          onFormChange={onFormChange}
        />
      ) : null}
      {step === "network" ? (
        <ContainerNetworkFields form={form} onFormChange={onFormChange} />
      ) : null}
      {step === "storage" ? (
        <ContainerStorageFields form={form} onFormChange={onFormChange} />
      ) : null}
      {step === "features" ? (
        <ContainerFeatureFields form={form} onFormChange={onFormChange} />
      ) : null}
    </div>
  );
}

function ContainerBasicFields({
  form,
  hostSelector,
  onFormChange,
}: {
  form: DockerFormState;
  hostSelector: ReactNode;
  onFormChange: (form: DockerFormState) => void;
}) {
  return (
    <div className="space-y-4">
      <div className="grid gap-4 sm:grid-cols-2">
        {hostSelector}
        <TextField
          label="容器名称"
          value={form.name}
          onChange={(name) => onFormChange({ ...form, name })}
          required
          placeholder="web"
        />
        <TextField
          label="镜像"
          value={form.image}
          onChange={(image) => onFormChange({ ...form, image })}
          required
          placeholder="nginx:1.27"
        />
        <TextField
          label="平台"
          value={form.platform}
          onChange={(platform) => onFormChange({ ...form, platform })}
          placeholder="linux/amd64"
        />
        <TextField
          label="Entrypoint"
          value={form.entrypoint}
          onChange={(entrypoint) => onFormChange({ ...form, entrypoint })}
          placeholder="/docker-entrypoint.sh"
        />
        <TextField
          label="启动命令"
          value={form.command}
          onChange={(command) => onFormChange({ ...form, command })}
          placeholder="nginx -g daemon off;"
        />
        <TextField
          label="主机名"
          value={form.hostname}
          onChange={(hostname) => onFormChange({ ...form, hostname })}
          placeholder="web-01"
        />
        <TextField
          label="域名"
          value={form.domainname}
          onChange={(domainname) => onFormChange({ ...form, domainname })}
          placeholder="home.arpa"
        />
        <TextField
          label="用户"
          value={form.user}
          onChange={(user) => onFormChange({ ...form, user })}
          placeholder="1000:1000"
        />
        <TextField
          label="工作目录"
          value={form.workingDir}
          onChange={(workingDir) => onFormChange({ ...form, workingDir })}
          placeholder="/app"
        />
      </div>
      <div className="grid gap-4 lg:grid-cols-2">
        <TextareaField
          label="环境变量"
          value={form.env}
          onChange={(env) => onFormChange({ ...form, env })}
          placeholder="KEY=value，每行一个"
          minHeight="min-h-28"
        />
        <TextareaField
          label="标签"
          value={form.labels}
          onChange={(labels) => onFormChange({ ...form, labels })}
          placeholder="key=value，每行一个"
          minHeight="min-h-28"
        />
      </div>
    </div>
  );
}

function ContainerNetworkFields({
  form,
  onFormChange,
}: {
  form: DockerFormState;
  onFormChange: (form: DockerFormState) => void;
}) {
  const networkNameLabel =
    form.networkMode === "container" ? "目标容器 ID 或名称" : "网络名称";
  const showNetworkName = form.networkMode === "custom" || form.networkMode === "container";

  return (
    <div className="space-y-4">
      <div className="grid gap-4 sm:grid-cols-2">
        <Field label="网络模式">
          <Select
            value={form.networkMode}
            onValueChange={(networkMode) => onFormChange({ ...form, networkMode })}
            options={[
              { value: "default", label: "默认" },
              { value: "bridge", label: "bridge" },
              { value: "host", label: "host" },
              { value: "none", label: "none" },
              { value: "custom", label: "自定义网络" },
              { value: "container", label: "container:<id>" },
            ]}
          />
        </Field>
        {showNetworkName ? (
          <TextField
            label={networkNameLabel}
            value={form.networkName}
            onChange={(networkName) => onFormChange({ ...form, networkName })}
            placeholder={form.networkMode === "container" ? "nginx" : "frontend"}
          />
        ) : (
          <TextField
            label="网络名称"
            value={form.networkName}
            onChange={(networkName) => onFormChange({ ...form, networkName })}
            placeholder="仅自定义网络需要"
            disabled
          />
        )}
        <TextField
          label="IPv4 地址"
          value={form.ipv4Address}
          onChange={(ipv4Address) => onFormChange({ ...form, ipv4Address })}
          placeholder="172.20.0.10"
        />
        <TextField
          label="MAC 地址"
          value={form.macAddress}
          onChange={(macAddress) => onFormChange({ ...form, macAddress })}
          placeholder="02:42:ac:14:00:0a"
        />
      </div>
      <div className="grid gap-4 lg:grid-cols-2">
        <TextareaField
          label="端口映射"
          value={form.ports}
          onChange={(ports) => onFormChange({ ...form, ports })}
          placeholder={"8080:80/tcp\n127.0.0.1:8443:443/tcp"}
          minHeight="min-h-28"
        />
        <TextareaField
          label="网络别名"
          value={form.aliases}
          onChange={(aliases) => onFormChange({ ...form, aliases })}
          placeholder="web，每行一个"
          minHeight="min-h-28"
        />
        <TextareaField
          label="DNS"
          value={form.dns}
          onChange={(dns) => onFormChange({ ...form, dns })}
          placeholder="1.1.1.1，每行一个"
          minHeight="min-h-24"
        />
        <TextareaField
          label="DNS Search"
          value={form.dnsSearch}
          onChange={(dnsSearch) => onFormChange({ ...form, dnsSearch })}
          placeholder="home.arpa，每行一个"
          minHeight="min-h-24"
        />
      </div>
      <TextareaField
        label="Extra Hosts"
        value={form.extraHosts}
        onChange={(extraHosts) => onFormChange({ ...form, extraHosts })}
        placeholder="host.docker.internal:host-gateway，每行一个"
        minHeight="min-h-24"
      />
    </div>
  );
}

function ContainerStorageFields({
  form,
  onFormChange,
}: {
  form: DockerFormState;
  onFormChange: (form: DockerFormState) => void;
}) {
  return (
    <div className="space-y-4">
      <div className="grid gap-4 lg:grid-cols-2">
        <TextareaField
          label="Bind / 命名卷"
          value={form.binds}
          onChange={(binds) => onFormChange({ ...form, binds })}
          placeholder={"/srv/www:/usr/share/nginx/html:ro\napp-data:/data"}
          minHeight="min-h-32"
        />
        <TextareaField
          label="匿名卷"
          value={form.volumes}
          onChange={(volumes) => onFormChange({ ...form, volumes })}
          placeholder="/cache，每行一个容器内路径"
          minHeight="min-h-32"
        />
        <TextareaField
          label="tmpfs"
          value={form.tmpfs}
          onChange={(tmpfs) => onFormChange({ ...form, tmpfs })}
          placeholder="/run:rw,size=64m，每行一个"
          minHeight="min-h-28"
        />
        <TextField
          label="/dev/shm 大小"
          value={form.shmSize}
          onChange={(shmSize) => onFormChange({ ...form, shmSize })}
          placeholder="64m"
        />
      </div>
    </div>
  );
}

function ContainerFeatureFields({
  form,
  onFormChange,
}: {
  form: DockerFormState;
  onFormChange: (form: DockerFormState) => void;
}) {
  return (
    <div className="space-y-5">
      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
        <Field label="重启策略">
          <Select
            value={form.restartPolicy}
            onValueChange={(restartPolicy) => onFormChange({ ...form, restartPolicy })}
            options={[
              { value: "default", label: "不设置" },
              { value: "no", label: "no" },
              { value: "always", label: "always" },
              { value: "unless-stopped", label: "unless-stopped" },
              { value: "on-failure", label: "on-failure" },
            ]}
          />
        </Field>
        <TextField
          label="最大重试次数"
          value={form.restartMaxRetries}
          onChange={(restartMaxRetries) => onFormChange({ ...form, restartMaxRetries })}
          placeholder="3"
          disabled={form.restartPolicy !== "on-failure"}
        />
        <TextField
          label="内存限制"
          value={form.memory}
          onChange={(memory) => onFormChange({ ...form, memory })}
          placeholder="512m"
        />
        <TextField
          label="Swap 限制"
          value={form.memorySwap}
          onChange={(memorySwap) => onFormChange({ ...form, memorySwap })}
          placeholder="1g 或 -1"
        />
        <TextField
          label="CPU"
          value={form.cpus}
          onChange={(cpus) => onFormChange({ ...form, cpus })}
          placeholder="0.5"
        />
        <TextField
          label="CPU Shares"
          value={form.cpuShares}
          onChange={(cpuShares) => onFormChange({ ...form, cpuShares })}
          placeholder="1024"
        />
        <TextField
          label="CPU Set"
          value={form.cpusetCpus}
          onChange={(cpusetCpus) => onFormChange({ ...form, cpusetCpus })}
          placeholder="0-3"
        />
        <TextField
          label="PID 限制"
          value={form.pidsLimit}
          onChange={(pidsLimit) => onFormChange({ ...form, pidsLimit })}
          placeholder="256"
        />
        <TextField
          label="日志驱动"
          value={form.logDriver}
          onChange={(logDriver) => onFormChange({ ...form, logDriver })}
          placeholder="json-file"
        />
      </div>

      <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
        <SwitchField label="自动删除" checked={form.autoRemove} onChange={(autoRemove) => onFormChange({ ...form, autoRemove })} />
        <SwitchField label="Privileged" checked={form.privileged} onChange={(privileged) => onFormChange({ ...form, privileged })} />
        <SwitchField label="Init" checked={form.init} onChange={(init) => onFormChange({ ...form, init })} />
        <SwitchField label="TTY" checked={form.tty} onChange={(tty) => onFormChange({ ...form, tty })} />
        <SwitchField label="Open stdin" checked={form.openStdin} onChange={(openStdin) => onFormChange({ ...form, openStdin })} />
        <SwitchField label="只读根文件系统" checked={form.readOnlyRootfs} onChange={(readOnlyRootfs) => onFormChange({ ...form, readOnlyRootfs })} />
      </div>

      <div className="grid gap-4 lg:grid-cols-2">
        <TextareaField
          label="Cap Add"
          value={form.capAdd}
          onChange={(capAdd) => onFormChange({ ...form, capAdd })}
          placeholder="NET_ADMIN，每行一个"
          minHeight="min-h-24"
        />
        <TextareaField
          label="Cap Drop"
          value={form.capDrop}
          onChange={(capDrop) => onFormChange({ ...form, capDrop })}
          placeholder="ALL，每行一个"
          minHeight="min-h-24"
        />
        <TextareaField
          label="设备"
          value={form.devices}
          onChange={(devices) => onFormChange({ ...form, devices })}
          placeholder="/dev/fuse:/dev/fuse:rwm，每行一个"
          minHeight="min-h-24"
        />
        <TextareaField
          label="日志选项"
          value={form.logOptions}
          onChange={(logOptions) => onFormChange({ ...form, logOptions })}
          placeholder="max-size=10m，每行一个"
          minHeight="min-h-24"
        />
      </div>

      <div className="space-y-4 rounded-md border bg-muted/20 p-4">
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
          <Field label="健康检查">
            <Select
              value={form.healthcheckMode}
              onValueChange={(healthcheckMode) => onFormChange({ ...form, healthcheckMode })}
              options={[
                { value: "inherit", label: "继承镜像" },
                { value: "command", label: "自定义命令" },
                { value: "disable", label: "禁用" },
              ]}
            />
          </Field>
          <TextField
            label="Interval 秒"
            value={form.healthcheckInterval}
            onChange={(healthcheckInterval) => onFormChange({ ...form, healthcheckInterval })}
            placeholder="30"
          />
          <TextField
            label="Timeout 秒"
            value={form.healthcheckTimeout}
            onChange={(healthcheckTimeout) => onFormChange({ ...form, healthcheckTimeout })}
            placeholder="5"
          />
          <TextField
            label="Retries"
            value={form.healthcheckRetries}
            onChange={(healthcheckRetries) => onFormChange({ ...form, healthcheckRetries })}
            placeholder="3"
          />
          <TextField
            label="Start Period 秒"
            value={form.healthcheckStartPeriod}
            onChange={(healthcheckStartPeriod) => onFormChange({ ...form, healthcheckStartPeriod })}
            placeholder="10"
          />
          <TextField
            label="Start Interval 秒"
            value={form.healthcheckStartInterval}
            onChange={(healthcheckStartInterval) => onFormChange({ ...form, healthcheckStartInterval })}
            placeholder="5"
          />
        </div>
        {form.healthcheckMode === "command" ? (
          <TextField
            label="健康检查命令"
            value={form.healthcheckCommand}
            onChange={(healthcheckCommand) => onFormChange({ ...form, healthcheckCommand })}
            placeholder="curl -f http://localhost/ || exit 1"
          />
        ) : null}
      </div>

      <div className="space-y-4 rounded-md border bg-muted/20 p-4">
        <SwitchField
          label="提交审批"
          checked={form.requiresApproval}
          onChange={(requiresApproval) => onFormChange({ ...form, requiresApproval })}
        />
        {form.requiresApproval ? (
          <TextareaField
            label="审批原因"
            value={form.reason}
            onChange={(reason) => onFormChange({ ...form, reason })}
            minHeight="min-h-20"
          />
        ) : null}
      </div>
    </div>
  );
}

function ContainerCreateFooter({
  form,
  stepIndex,
  hosts,
  onOpenChange,
  onStepChange,
}: {
  form: DockerFormState;
  stepIndex: number;
  hosts: Host[];
  onOpenChange: (open: boolean) => void;
  onStepChange: (step: ContainerCreateStep) => void;
}) {
  const isFirstStep = stepIndex === 0;
  const isLastStep = stepIndex === containerCreateSteps.length - 1;
  const canSubmit = containerCreateCanSubmit(form, hosts);
  const nextStep = containerCreateSteps[Math.min(stepIndex + 1, containerCreateSteps.length - 1)];
  const previousStep = containerCreateSteps[Math.max(stepIndex - 1, 0)];

  return (
    <DialogFooter className="border-t pt-4 sm:items-center sm:justify-between sm:space-x-0">
      <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
        取消
      </Button>
      <div className="flex flex-col-reverse gap-2 sm:flex-row">
        <Button
          type="button"
          variant="outline"
          disabled={isFirstStep}
          onClick={() => onStepChange(previousStep.value)}
        >
          <ChevronLeft className="size-4" aria-hidden="true" />
          上一步
        </Button>
        {isLastStep ? (
          <Button type="submit" disabled={!canSubmit}>
            <Check className="size-4" aria-hidden="true" />
            {form.requiresApproval ? "提交审批" : "直接创建"}
          </Button>
        ) : (
          <Button
            type="button"
            disabled={stepIndex === 0 && !canSubmit}
            onClick={() => onStepChange(nextStep.value)}
          >
            下一步
            <ChevronRight className="size-4" aria-hidden="true" />
          </Button>
        )}
      </div>
    </DialogFooter>
  );
}

function TextareaField({
  label,
  value,
  onChange,
  placeholder,
  minHeight = "min-h-20",
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  minHeight?: string;
}) {
  return (
    <Field label={label}>
      <textarea
        value={value}
        onChange={(event) => onChange(event.target.value)}
        placeholder={placeholder}
        className={`${minHeight} w-full rounded-md border bg-background px-3 py-2 text-sm outline-none ring-offset-background focus-visible:ring-2 focus-visible:ring-ring`}
      />
    </Field>
  );
}

function SwitchField({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <label className="flex h-10 items-center justify-between gap-3 rounded-md border bg-background px-3 text-sm">
      <span className="font-medium">{label}</span>
      <Switch checked={checked} onCheckedChange={onChange} />
    </label>
  );
}

function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <label className="block space-y-2 text-sm">
      <span className="font-medium">{label}</span>
      {children}
    </label>
  );
}

function TextField({
  label,
  value,
  onChange,
  required,
  disabled,
  placeholder,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  required?: boolean;
  disabled?: boolean;
  placeholder?: string;
}) {
  return (
    <Field label={label}>
      <input
        required={required}
        disabled={disabled}
        value={value}
        onChange={(event) => onChange(event.target.value)}
        placeholder={placeholder}
        className="h-9 w-full rounded-md border bg-background px-3 text-sm outline-none ring-offset-background focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-60"
      />
    </Field>
  );
}

function CheckboxField({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <label className="flex h-9 items-center gap-2 rounded-md border bg-background px-3 text-sm">
      <input
        type="checkbox"
        checked={checked}
        onChange={(event) => onChange(event.target.checked)}
        className="size-4"
      />
      <span>{label}</span>
    </label>
  );
}

async function loadDockerRows(kind: DockerKind, hostId?: string) {
  if (kind === "containers") {
    return getDockerContainers(hostId);
  }
  if (kind === "images") {
    return getDockerImages(hostId);
  }
  if (kind === "networks") {
    return getDockerNetworks(hostId);
  }
  if (kind === "volumes") {
    return getDockerVolumes(hostId);
  }
  return getDockerComposeProjects(hostId);
}

function toRows(
  kind: DockerKind,
  items: Array<
    DockerContainerSummary
    | DockerImageSummary
    | DockerNetworkSummary
    | DockerVolumeSummary
    | DockerComposeProject
  >,
  hostNames: Map<string, string>,
): DockerRow[] {
  if (kind === "containers") {
    return (items as DockerContainerSummary[]).map((item) => {
      const name = item.names[0]?.replace(/^\//, "") || item.id || "unknown";
      const state = item.state ?? "unknown";
      return {
        id: `${item.host_id}:${item.id ?? name}`,
        hostId: item.host_id,
        hostLabel: hostNames.get(item.host_id) ?? item.host_id,
        name,
        secondary: item.id ?? "-",
        status: dockerResourceStatus(state),
        statusLabel: item.status ?? state,
        detailA: item.image ?? "-",
        detailB: formatPorts(item.ports),
        detailC: item.status ?? "-",
        raw: item,
      };
    });
  }
  if (kind === "images") {
    return (items as DockerImageSummary[]).map((item) => {
      const name = item.repo_tags[0] ?? item.id ?? "untagged";
      return {
        id: `${item.host_id}:${item.id ?? name}`,
        hostId: item.host_id,
        hostLabel: hostNames.get(item.host_id) ?? item.host_id,
        name,
        secondary: item.id ?? item.repo_digests[0] ?? "-",
        status: "running",
        statusLabel: "available",
        detailA: item.repo_tags.length ? item.repo_tags.join(", ") : "untagged",
        detailB: formatBytes(item.size),
        detailC: item.created ? formatUnixTime(item.created) : "-",
        raw: item,
      };
    });
  }
  if (kind === "networks") {
    return (items as DockerNetworkSummary[]).map((item) => {
      const name = item.name ?? item.id ?? "unknown";
      return {
        id: `${item.host_id}:${item.id ?? name}`,
        hostId: item.host_id,
        hostLabel: hostNames.get(item.host_id) ?? item.host_id,
        name,
        secondary: item.id ?? "-",
        status: "running",
        statusLabel: item.driver ?? "network",
        detailA: item.driver ?? "-",
        detailB: item.scope ?? "-",
        detailC: [
          item.internal ? "internal" : null,
          item.attachable ? "attachable" : null,
          item.ingress ? "ingress" : null,
        ].filter(Boolean).join(", ") || "-",
        raw: item,
      };
    });
  }
  if (kind === "volumes") {
    return (items as DockerVolumeSummary[]).map((item) => ({
      id: `${item.host_id}:${item.name}`,
      hostId: item.host_id,
      hostLabel: hostNames.get(item.host_id) ?? item.host_id,
      name: item.name,
      secondary: item.mountpoint ?? "-",
      status: "running",
      statusLabel: item.driver ?? "volume",
      detailA: item.driver ?? "-",
      detailB: formatBytes(item.usage_size),
      detailC: item.usage_ref_count == null ? "-" : `${item.usage_ref_count} 引用`,
      raw: item,
    }));
  }
  return (items as DockerComposeProject[]).map((item) => ({
    id: `${item.host_id}:${item.name}`,
    hostId: item.host_id,
    hostLabel: hostNames.get(item.host_id) ?? item.host_id,
    name: item.name,
    secondary: item.path,
    status: item.status === "configured" ? "running" : "warning",
    statusLabel: item.status,
    detailA: item.services.join(", ") || "-",
    detailB: item.status,
    detailC: item.path,
    raw: item,
  }));
}

async function runDialogSubmit(dialog: NonNullable<ActiveDialog>, form: DockerFormState) {
  const reason = form.reason.trim() || null;
  if (dialog.kind === "container-create") {
    return createDockerContainer({
      host_id: form.hostId,
      execution_mode: form.requiresApproval ? "approval" : "direct",
      name: form.name.trim(),
      image: form.image.trim(),
      platform: optionalText(form.platform),
      hostname: optionalText(form.hostname),
      domainname: optionalText(form.domainname),
      user: optionalText(form.user),
      working_dir: optionalText(form.workingDir),
      entrypoint: splitWords(form.entrypoint),
      command: splitWords(form.command),
      env: splitLines(form.env),
      labels: keyValueObject(form.labels),
      network_mode: containerNetworkMode(form),
      network_name: form.networkMode === "custom" ? optionalText(form.networkName) : null,
      aliases: splitLines(form.aliases),
      ipv4_address: optionalText(form.ipv4Address),
      mac_address: optionalText(form.macAddress),
      ports: parsePortLines(form.ports),
      dns: splitLines(form.dns),
      dns_search: splitLines(form.dnsSearch),
      extra_hosts: splitLines(form.extraHosts),
      binds: splitLines(form.binds),
      volumes: splitLines(form.volumes),
      tmpfs: splitLines(form.tmpfs),
      shm_size: optionalText(form.shmSize),
      restart_policy: containerRestartPolicy(form.restartPolicy),
      restart_max_retries: optionalInteger(form.restartMaxRetries),
      auto_remove: form.autoRemove,
      privileged: form.privileged,
      init: form.init,
      tty: form.tty,
      open_stdin: form.openStdin,
      read_only_rootfs: form.readOnlyRootfs,
      cap_add: splitLines(form.capAdd),
      cap_drop: splitLines(form.capDrop),
      devices: parseDeviceLines(form.devices),
      memory: optionalText(form.memory),
      memory_swap: optionalText(form.memorySwap),
      cpus: optionalText(form.cpus),
      cpu_shares: optionalInteger(form.cpuShares),
      cpuset_cpus: optionalText(form.cpusetCpus),
      pids_limit: optionalInteger(form.pidsLimit),
      healthcheck: containerHealthcheck(form),
      log_driver: optionalText(form.logDriver),
      log_options: keyValueObject(form.logOptions),
      reason: form.requiresApproval ? reason : null,
    });
  }
  if (dialog.kind === "image-pull") {
    return pullDockerImage({
      host_id: form.hostId,
      reference: form.reference.trim(),
      tag: optionalText(form.tag),
      platform: optionalText(form.platform),
      reason,
    });
  }
  if (dialog.kind === "image-remove") {
    return removeDockerImage({
      host_id: form.hostId,
      reference: form.reference.trim(),
      force: form.force,
      noprune: form.noprune,
      reason,
    });
  }
  if (dialog.kind === "network-create") {
    return createDockerNetwork({
      host_id: form.hostId,
      name: form.name.trim(),
      driver: form.driver.trim() || "bridge",
      internal: form.internal,
      attachable: form.attachable,
      labels: keyValueObject(form.labels),
      reason,
    });
  }
  if (dialog.kind === "network-connect" || dialog.kind === "network-disconnect") {
    return dockerNetworkContainerAction(
      form.name.trim(),
      dialog.kind === "network-connect" ? "connect" : "disconnect",
      {
        host_id: form.hostId,
        container: form.container.trim(),
        force: form.force,
        reason,
      },
    );
  }
  if (dialog.kind === "volume-create") {
    return createDockerVolume({
      host_id: form.hostId,
      name: form.name.trim(),
      driver: form.driver.trim() || "local",
      driver_opts: keyValueObject(form.driverOpts),
      labels: keyValueObject(form.labels),
      reason,
    });
  }

  const request = {
    host_id: form.hostId,
    name: form.name.trim(),
    compose_yaml: form.composeYaml,
    env_file: optionalText(form.envFile),
    reason,
  };
  return dialog.row
    ? updateDockerComposeProject(form.name.trim(), request)
    : createDockerComposeProject(request);
}

function defaultCreateDialog(kind: DockerKind): DialogKind {
  if (kind === "containers") {
    return "container-create";
  }
  if (kind === "images") {
    return "image-pull";
  }
  if (kind === "networks") {
    return "network-create";
  }
  if (kind === "volumes") {
    return "volume-create";
  }
  return "compose-edit";
}

function defaultReason(kind: DialogKind, row?: DockerRow) {
  const target = row ? ` ${row.name}` : "";
  return `operator requested docker ${kind.replaceAll("-", " ")}${target}`;
}

function dialogTitle(kind?: DialogKind) {
  if (kind === "container-create") {
    return "创建容器";
  }
  if (kind === "image-pull") {
    return "拉取镜像";
  }
  if (kind === "image-remove") {
    return "移除镜像";
  }
  if (kind === "network-create") {
    return "创建网络";
  }
  if (kind === "network-connect") {
    return "连接容器到网络";
  }
  if (kind === "network-disconnect") {
    return "断开容器网络";
  }
  if (kind === "volume-create") {
    return "创建存储卷";
  }
  if (kind === "compose-edit") {
    return "编辑 Compose 项目";
  }
  return "Docker 操作";
}

function dialogDescription(kind?: DialogKind) {
  if (kind === "container-create") {
    return "默认直接创建并记录任务审计；需要人工确认时可在特性阶段打开提交审批。";
  }
  if (kind === "compose-edit") {
    return "Compose 文件只会写入 Agent 配置的受控目录，部署动作仍需要审批。";
  }
  return "该操作会创建高风险审批任务，批准后由目标 Agent 执行。";
}

function dockerResourceStatus(state: string): ResourceStatus {
  if (state === "running") {
    return "running";
  }
  if (state === "created" || state === "restarting" || state === "paused") {
    return "warning";
  }
  return "stopped";
}

function hostLabel(host: Host) {
  return host.display_name || host.hostname;
}

function shortId(value: string) {
  return value.slice(0, 8);
}

function optionalText(value: string) {
  const trimmed = value.trim();
  return trimmed ? trimmed : null;
}

function splitLines(value: string) {
  return value
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean);
}

function splitWords(value: string) {
  return value.trim() ? value.trim().split(/\s+/) : [];
}

function keyValueObject(value: string) {
  return Object.fromEntries(
    splitLines(value).flatMap((line) => {
      const index = line.indexOf("=");
      if (index <= 0) {
        return [];
      }
      return [[line.slice(0, index).trim(), line.slice(index + 1).trim()]];
    }),
  );
}

function containerCreateCanSubmit(form: DockerFormState, hosts: Host[]) {
  return Boolean(
    form.hostId &&
      hosts.length > 0 &&
      form.name.trim() &&
      form.image.trim(),
  );
}

function containerNetworkMode(form: DockerFormState) {
  if (form.networkMode === "default") {
    return null;
  }
  if (form.networkMode === "custom") {
    return optionalText(form.networkName);
  }
  if (form.networkMode === "container") {
    const target = optionalText(form.networkName);
    return target ? `container:${target}` : null;
  }
  return form.networkMode;
}

function containerRestartPolicy(value: string): DockerContainerRestartPolicyName | null {
  if (
    value === "no" ||
    value === "always" ||
    value === "unless-stopped" ||
    value === "on-failure"
  ) {
    return value;
  }
  return null;
}

function containerHealthcheck(form: DockerFormState) {
  if (form.healthcheckMode === "inherit") {
    return null;
  }
  return {
    disabled: form.healthcheckMode === "disable",
    command:
      form.healthcheckMode === "command"
        ? optionalText(form.healthcheckCommand)
        : null,
    interval_seconds: optionalInteger(form.healthcheckInterval),
    timeout_seconds: optionalInteger(form.healthcheckTimeout),
    retries: optionalInteger(form.healthcheckRetries),
    start_period_seconds: optionalInteger(form.healthcheckStartPeriod),
    start_interval_seconds: optionalInteger(form.healthcheckStartInterval),
  };
}

function parsePortLines(value: string) {
  return splitLines(value).map((line) => {
    const parts = line.split(":");
    let hostIp: string | null = null;
    let hostPort: string | null = null;
    let target = line;
    if (parts.length === 2) {
      hostPort = optionalText(parts[0]);
      target = parts[1];
    } else if (parts.length >= 3) {
      hostIp = optionalText(parts[0]);
      hostPort = optionalText(parts[1]);
      target = parts.slice(2).join(":");
    }
    const [containerPort, protocol] = target.split("/", 2);
    return {
      container_port: containerPort.trim(),
      protocol: optionalText(protocol ?? ""),
      host_ip: hostIp,
      host_port: hostPort,
    };
  });
}

function parseDeviceLines(value: string) {
  return splitLines(value).map((line) => {
    const [hostPath, containerPath, permissions] = line.split(":", 3);
    return {
      host_path: hostPath.trim(),
      container_path: optionalText(containerPath ?? ""),
      permissions: optionalText(permissions ?? ""),
    };
  });
}

function optionalInteger(value: string) {
  const trimmed = value.trim();
  if (!trimmed) {
    return null;
  }
  const numberValue = Number.parseInt(trimmed, 10);
  return Number.isFinite(numberValue) ? numberValue : null;
}

function formatUnixTime(value: bigint | number) {
  const numberValue = Number(value);
  if (!Number.isFinite(numberValue) || numberValue <= 0) {
    return "-";
  }
  return new Date(numberValue * 1000).toLocaleString("zh-CN");
}

function formatBytes(value: bigint | number | null) {
  if (value == null) {
    return "-";
  }
  const numberValue = Number(value);
  if (!Number.isFinite(numberValue) || numberValue <= 0) {
    return "0 B";
  }
  const units = ["B", "KB", "MB", "GB", "TB"];
  let size = numberValue;
  let unit = 0;
  while (size >= 1024 && unit < units.length - 1) {
    size /= 1024;
    unit += 1;
  }
  return `${size.toFixed(unit === 0 ? 0 : 1)} ${units[unit]}`;
}

function formatPorts(value: unknown) {
  if (!Array.isArray(value) || value.length === 0) {
    return "-";
  }
  return value
    .map((port) => {
      if (!port || typeof port !== "object") {
        return null;
      }
      const record = port as Record<string, unknown>;
      const privatePort = record.PrivatePort ?? record.private_port;
      const publicPort = record.PublicPort ?? record.public_port;
      return publicPort ? `${publicPort}:${privatePort}` : String(privatePort ?? "-");
    })
    .filter(Boolean)
    .join(", ");
}
