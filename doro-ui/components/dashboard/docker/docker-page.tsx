"use client";

import {
  Cable,
  Check,
  ChevronLeft,
  ChevronRight,
  FilePenLine,
  Filter,
  FolderOpen,
  KeyRound,
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
import {
  KeyValueRowsField,
  StringListRowsField,
  type KeyValueRow,
  type StringListRow,
} from "@/components/admin/repeating-fields";
import { Toolbar } from "@/components/admin/toolbar";
import { HostDirectoryPickerDialog } from "@/components/dashboard/files/host-directory-picker-dialog";
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
  getDockerRegistryCredentials,
  getDockerVolumes,
  getHosts,
  pullDockerImage,
  readDockerComposeProject,
  removeDockerImage,
  removeDockerRegistryCredential,
  removeDockerVolume,
  updateDockerComposeProject,
  upsertDockerRegistryCredential,
} from "@/lib/control-plane-api";
import type {
  DockerActionResponse,
  DockerComposeProject,
  DockerContainerRestartPolicyName,
  DockerContainerSummary,
  DockerImageSummary,
  DockerNetworkSummary,
  DockerRegistryCredentialSummary,
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

type PortMappingRow = {
  id: string;
  hostIp: string;
  hostPort: string;
  containerPort: string;
  protocol: string;
};

type BindMountRow = {
  id: string;
  source: string;
  target: string;
  mode: string;
};

type AnonymousVolumeRow = {
  id: string;
  target: string;
};

type TmpfsMountRow = {
  id: string;
  target: string;
  options: string;
};

type ExtraHostRow = {
  id: string;
  hostname: string;
  address: string;
};

type DeviceMappingRow = {
  id: string;
  hostPath: string;
  containerPath: string;
  permissions: string;
};

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
  containerEnv: KeyValueRow[];
  operationLabels: KeyValueRow[];
  containerLabels: KeyValueRow[];
  networkMode: string;
  networkName: string;
  containerAliases: StringListRow[];
  ipv4Address: string;
  macAddress: string;
  containerPorts: PortMappingRow[];
  containerDns: StringListRow[];
  containerDnsSearch: StringListRow[];
  containerExtraHosts: ExtraHostRow[];
  containerBinds: BindMountRow[];
  containerVolumes: AnonymousVolumeRow[];
  containerTmpfs: TmpfsMountRow[];
  shmSize: string;
  restartPolicy: string;
  restartMaxRetries: string;
  autoRemove: boolean;
  privileged: boolean;
  init: boolean;
  tty: boolean;
  openStdin: boolean;
  readOnlyRootfs: boolean;
  containerCapAdd: StringListRow[];
  containerCapDrop: StringListRow[];
  containerDevices: DeviceMappingRow[];
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
  containerLogOptions: KeyValueRow[];
  driver: string;
  internal: boolean;
  attachable: boolean;
  driverOptionRows: KeyValueRow[];
  force: boolean;
  noprune: boolean;
  container: string;
  composeYaml: string;
  envFile: string;
  reason: string;
};

type RegistryFormState = {
  hostId: string;
  registry: string;
  username: string;
  secret: string;
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
  containerEnv: [],
  operationLabels: [],
  containerLabels: [],
  networkMode: "bridge",
  networkName: "",
  containerAliases: [],
  ipv4Address: "",
  macAddress: "",
  containerPorts: [],
  containerDns: [],
  containerDnsSearch: [],
  containerExtraHosts: [],
  containerBinds: [],
  containerVolumes: [],
  containerTmpfs: [],
  shmSize: "",
  restartPolicy: "default",
  restartMaxRetries: "",
  autoRemove: false,
  privileged: false,
  init: false,
  tty: false,
  openStdin: false,
  readOnlyRootfs: false,
  containerCapAdd: [],
  containerCapDrop: [],
  containerDevices: [],
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
  containerLogOptions: [],
  driver: "",
  internal: false,
  attachable: false,
  driverOptionRows: [],
  force: true,
  noprune: false,
  container: "",
  composeYaml: defaultComposeYaml,
  envFile: "",
  reason: "",
};

const emptyRegistryForm: RegistryFormState = {
  hostId: "",
  registry: "",
  username: "",
  secret: "",
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
    description: "实时查询 Docker 镜像，拉取镜像直接执行并保留审计，移除镜像仍需审批。",
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

const baseColumns: ResourceColumn<DockerRow>[] = [
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
    render: (row) =>
      isImageRow(row) ? (
        <TruncatedText value={row.statusLabel} />
      ) : (
        <ResourceStatusBadge status={row.status} />
      ),
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

function dockerColumns(kind: DockerKind) {
  if (kind !== "images") {
    return baseColumns;
  }
  return baseColumns.map((column) =>
    column.key === "status"
      ? { ...column, label: "架构" }
      : column.key === "detailC"
        ? { ...column, label: "创建时间" }
        : column,
  );
}

function isImageRow(row: DockerRow) {
  return "repo_tags" in row.raw;
}

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
  const [registryDialogOpen, setRegistryDialogOpen] = useState(false);
  const [registryCredentialDialogOpen, setRegistryCredentialDialogOpen] = useState(false);
  const [registryCredentialMode, setRegistryCredentialMode] = useState<"create" | "edit">("create");
  const [registryCredentials, setRegistryCredentials] = useState<DockerRegistryCredentialSummary[]>([]);
  const [registryForm, setRegistryForm] = useState<RegistryFormState>(emptyRegistryForm);
  const [registryLoading, setRegistryLoading] = useState(false);
  const [registryPending, setRegistryPending] = useState<string | null>(null);
  const meta = kindMeta[kind];
  const columns = useMemo(() => dockerColumns(kind), [kind]);

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

  const loadRegistryCredentials = useCallback(async (hostId: string) => {
    if (!hostId) {
      setRegistryCredentials([]);
      return;
    }
    setRegistryLoading(true);
    const result = await getDockerRegistryCredentials(hostId);
    if (result.data) {
      setRegistryCredentials(result.data.items);
      setApiError(null);
    } else {
      setRegistryCredentials([]);
      setApiError(result.error);
    }
    setRegistryLoading(false);
  }, []);

  useEffect(() => {
    if (registryDialogOpen && registryForm.hostId) {
      void loadRegistryCredentials(registryForm.hostId);
    }
  }, [loadRegistryCredentials, registryDialogOpen, registryForm.hostId]);

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

  const openRegistryDialog = () => {
    const hostId = hostFilter !== "all" ? hostFilter : dockerHosts[0]?.id ?? "";
    setRegistryForm({ ...emptyRegistryForm, hostId });
    setRegistryCredentials([]);
    setRegistryDialogOpen(true);
  };

  const updateRegistryDialogOpen = (open: boolean) => {
    setRegistryDialogOpen(open);
    if (!open) {
      setRegistryCredentialDialogOpen(false);
      setRegistryPending(null);
    }
  };

  const openRegistryCredentialForm = (credential?: DockerRegistryCredentialSummary) => {
    if (credential) {
      setRegistryCredentialMode("edit");
      setRegistryForm({
        hostId: credential.host_id,
        registry: credential.registry,
        username: credential.username ?? "",
        secret: "",
      });
    } else {
      setRegistryCredentialMode("create");
      setRegistryForm({
        ...emptyRegistryForm,
        hostId: registryForm.hostId,
      });
    }
    setRegistryCredentialDialogOpen(true);
  };

  const submitRegistryCredential = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!registryForm.hostId) {
      return;
    }
    setRegistryPending("save");
    setNotice(null);
    const result = await upsertDockerRegistryCredential({
      host_id: registryForm.hostId,
      registry: registryForm.registry.trim(),
      username: registryForm.username.trim(),
      secret: registryForm.secret,
    });
    if (result.data) {
      setNotice(`已更新 Registry 凭证：${result.data.item.registry}`);
      setRegistryCredentialDialogOpen(false);
      setRegistryForm({
        ...emptyRegistryForm,
        hostId: registryForm.hostId,
      });
      await loadRegistryCredentials(registryForm.hostId);
    } else {
      setApiError(result.error);
    }
    setRegistryPending(null);
  };

  const removeRegistryCredential = async (credential: DockerRegistryCredentialSummary) => {
    if (!window.confirm(`删除 ${credential.registry} 的 Registry 凭证？`)) {
      return;
    }
    setRegistryPending(`remove:${credential.registry}`);
    setNotice(null);
    const result = await removeDockerRegistryCredential({
      host_id: credential.host_id,
      registry: credential.registry,
    });
    if (result.data) {
      setNotice(`已删除 Registry 凭证：${result.data.item.registry}`);
      await loadRegistryCredentials(credential.host_id);
      if (registryCredentialDialogOpen && registryForm.registry === credential.registry) {
        setRegistryCredentialDialogOpen(false);
        setRegistryForm({ ...emptyRegistryForm, hostId: credential.host_id });
      }
    } else {
      setApiError(result.error);
    }
    setRegistryPending(null);
  };

  const submitDialog = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!activeDialog) {
      return;
    }
    setBusyId(activeDialog.row?.id ?? activeDialog.kind);
    setNotice(null);
    const result = await runDialogSubmit(activeDialog, form);
    handleActionResult(result, dialogSubmitMessage(activeDialog.kind, form));
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
    handleActionResult(result, composeActionMessage(action));
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
            <div className="flex flex-wrap gap-2">
              <Button onClick={() => openDialog(defaultCreateDialog(kind))}>
                <Plus className="size-4" aria-hidden="true" />
                {meta.createLabel}
              </Button>
              {kind === "images" ? (
                <Button
                  variant="outline"
                  disabled={dockerHosts.length === 0}
                  onClick={openRegistryDialog}
                >
                  <KeyRound className="size-4" aria-hidden="true" />
                  Registry 管理
                </Button>
              ) : null}
            </div>
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
      <RegistryCredentialsDialog
        open={registryDialogOpen}
        hosts={dockerHosts}
        form={registryForm}
        credentials={registryCredentials}
        loading={registryLoading}
        pending={registryPending}
        onOpenChange={updateRegistryDialogOpen}
        onFormChange={setRegistryForm}
        onAdd={() => openRegistryCredentialForm()}
        onEdit={openRegistryCredentialForm}
        onRemove={removeRegistryCredential}
      />
      <RegistryCredentialFormDialog
        open={registryCredentialDialogOpen}
        mode={registryCredentialMode}
        form={registryForm}
        pending={registryPending}
        onOpenChange={setRegistryCredentialDialogOpen}
        onFormChange={setRegistryForm}
        onSubmit={submitRegistryCredential}
      />
    </PageContainer>
  );
}

function RegistryCredentialsDialog({
  open,
  hosts,
  form,
  credentials,
  loading,
  pending,
  onOpenChange,
  onFormChange,
  onAdd,
  onEdit,
  onRemove,
}: {
  open: boolean;
  hosts: Host[];
  form: RegistryFormState;
  credentials: DockerRegistryCredentialSummary[];
  loading: boolean;
  pending: string | null;
  onOpenChange: (open: boolean) => void;
  onFormChange: (form: RegistryFormState) => void;
  onAdd: () => void;
  onEdit: (credential: DockerRegistryCredentialSummary) => void;
  onRemove: (credential: DockerRegistryCredentialSummary) => void;
}) {
  const operationPending = pending !== null;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-4xl">
        <div className="space-y-5">
          <DialogHeader>
            <DialogTitle>Registry 管理</DialogTitle>
            <DialogDescription>
              管理目标 Agent 用户默认 Docker 配置中的 registry 凭证，镜像拉取和 Compose 会使用这些凭证。
            </DialogDescription>
          </DialogHeader>

          <div className="grid gap-3 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-end">
            <Field label="目标 Agent">
              <Select
                required
                value={form.hostId}
                disabled={hosts.length === 0 || operationPending}
                onValueChange={(hostId) => onFormChange({ ...emptyRegistryForm, hostId })}
                options={
                  hosts.length === 0
                    ? [{ value: "", label: "暂无在线 Docker Agent" }]
                    : hosts.map((host) => ({ value: host.id, label: hostLabel(host) }))
                }
              />
            </Field>
            <Button type="button" disabled={!form.hostId || operationPending} onClick={onAdd}>
              <Plus className="size-4" aria-hidden="true" />
              新增凭证
            </Button>
          </div>

          <div className="space-y-2">
            <div className="flex items-center justify-between gap-3">
              <h3 className="text-sm font-medium">已保存凭证</h3>
              <span className="truncate text-xs text-muted-foreground">
                {credentials[0]?.config_path ?? "~/.docker/config.json"}
              </span>
            </div>
            <div className="overflow-hidden rounded-md border">
              {loading ? (
                <div className="px-3 py-6 text-center text-sm text-muted-foreground">
                  正在读取 Registry 凭证...
                </div>
              ) : credentials.length === 0 ? (
                <div className="px-3 py-6 text-center text-sm text-muted-foreground">
                  暂无 Registry 凭证
                </div>
              ) : (
                <div className="divide-y">
                  {credentials.map((credential) => {
                    const removePending = pending === `remove:${credential.registry}`;
                    const external = credential.source !== "inline";
                    return (
                      <div
                        key={`${credential.host_id}:${credential.registry}`}
                        className="grid gap-3 px-3 py-3 sm:grid-cols-[minmax(0,1.4fr)_minmax(0,1fr)_auto]"
                      >
                        <div className="min-w-0">
                          <p className="truncate font-medium" title={credential.registry}>
                            {credential.registry}
                          </p>
                          <p className="truncate text-xs text-muted-foreground">
                            {registrySourceLabel(credential.source)}
                          </p>
                        </div>
                        <div className="min-w-0 text-sm">
                          <p className="truncate" title={credential.username ?? ""}>
                            {credential.username ?? "-"}
                          </p>
                          <p className="truncate text-xs text-muted-foreground">
                            {credential.has_secret ? "已保存凭证" : "未保存 inline 凭证"}
                          </p>
                        </div>
                        <div className="flex items-center gap-2">
                          <Button
                            type="button"
                            variant="outline"
                            size="sm"
                            disabled={operationPending || removePending}
                            onClick={() => onEdit(credential)}
                          >
                            <FilePenLine className="size-4" aria-hidden="true" />
                            编辑
                          </Button>
                          <Button
                            type="button"
                            variant="outline"
                            size="sm"
                            disabled={operationPending || removePending || external}
                            onClick={() => onRemove(credential)}
                          >
                            <Trash2 className="size-4" aria-hidden="true" />
                            删除
                          </Button>
                        </div>
                      </div>
                    );
                  })}
                </div>
              )}
            </div>
          </div>

          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
              关闭
            </Button>
          </DialogFooter>
        </div>
      </DialogContent>
    </Dialog>
  );
}

function RegistryCredentialFormDialog({
  open,
  mode,
  form,
  pending,
  onOpenChange,
  onFormChange,
  onSubmit,
}: {
  open: boolean;
  mode: "create" | "edit";
  form: RegistryFormState;
  pending: string | null;
  onOpenChange: (open: boolean) => void;
  onFormChange: (form: RegistryFormState) => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
}) {
  const savePending = pending === "save";
  const canSave =
    Boolean(form.hostId) &&
    Boolean(form.registry.trim()) &&
    Boolean(form.username.trim()) &&
    Boolean(form.secret);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-xl">
        <form onSubmit={onSubmit} className="space-y-5">
          <DialogHeader>
            <DialogTitle>{mode === "edit" ? "编辑 Registry 凭证" : "新增 Registry 凭证"}</DialogTitle>
            <DialogDescription>
              保存到目标 Agent 用户默认 Docker 配置，列表不会显示密码或 Token。
            </DialogDescription>
          </DialogHeader>

          <div className="grid gap-4 sm:grid-cols-2">
            <TextField
              label="Registry"
              value={form.registry}
              disabled={savePending || !form.hostId || mode === "edit"}
              onChange={(registry) => onFormChange({ ...form, registry })}
              required
              placeholder="docker.io 或 ghcr.io"
            />
            <TextField
              label="用户名"
              value={form.username}
              disabled={savePending || !form.hostId}
              onChange={(username) => onFormChange({ ...form, username })}
              required
              placeholder="registry 用户名"
            />
            <div className="sm:col-span-2">
              <TextField
                label="密码或 Token"
                value={form.secret}
                disabled={savePending || !form.hostId}
                onChange={(secret) => onFormChange({ ...form, secret })}
                required
                type="password"
                placeholder="保存时需重新输入"
              />
            </div>
          </div>

          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
              取消
            </Button>
            <Button type="submit" disabled={!canSave || savePending}>
              <KeyRound className="size-4" aria-hidden="true" />
              保存凭证
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
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
  const showApprovalReason =
    Boolean(kind) && !isContainerCreate && kind !== "image-pull" && kind !== "compose-edit";
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

          {showApprovalReason ? (
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
                {dialogSubmitLabel(kind, dialog?.row)}
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
        <KeyValueRowsField
          label="标签"
          rows={form.operationLabels}
          keyPlaceholder="com.example.scope"
          valuePlaceholder="network"
          addLabel="添加标签"
          emptyText="暂无标签"
          onChange={(operationLabels) => onFormChange({ ...form, operationLabels })}
        />
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
          <KeyValueRowsField
            label="Driver opts"
            rows={form.driverOptionRows}
            keyPlaceholder="type"
            valuePlaceholder="none"
            addLabel="添加选项"
            emptyText="暂无 Driver opts"
            onChange={(driverOptionRows) => onFormChange({ ...form, driverOptionRows })}
          />
          <KeyValueRowsField
            label="标签"
            rows={form.operationLabels}
            keyPlaceholder="com.example.scope"
            valuePlaceholder="volume"
            addLabel="添加标签"
            emptyText="暂无标签"
            onChange={(operationLabels) => onFormChange({ ...form, operationLabels })}
          />
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
  const selectedHost = hosts.find((host) => host.id === form.hostId) ?? null;
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
        <ContainerStorageFields
          form={form}
          selectedHost={selectedHost}
          onFormChange={onFormChange}
        />
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
        <KeyValueRowsField
          label="环境变量"
          rows={form.containerEnv}
          keyPlaceholder="POSTGRES_PASSWORD"
          valuePlaceholder="secret"
          addLabel="添加变量"
          emptyText="暂无环境变量"
          onChange={(containerEnv) => onFormChange({ ...form, containerEnv })}
        />
        <KeyValueRowsField
          label="标签"
          rows={form.containerLabels}
          keyPlaceholder="com.example.role"
          valuePlaceholder="web"
          addLabel="添加标签"
          emptyText="暂无标签"
          onChange={(containerLabels) => onFormChange({ ...form, containerLabels })}
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
      <PortMappingsField
        rows={form.containerPorts}
        onChange={(containerPorts) => onFormChange({ ...form, containerPorts })}
      />
      <div className="grid gap-4 lg:grid-cols-2">
        <StringListRowsField
          label="网络别名"
          rows={form.containerAliases}
          valuePlaceholder="web"
          addLabel="添加别名"
          emptyText="暂无网络别名"
          onChange={(containerAliases) => onFormChange({ ...form, containerAliases })}
        />
        <StringListRowsField
          label="DNS"
          rows={form.containerDns}
          valuePlaceholder="1.1.1.1"
          addLabel="添加 DNS"
          emptyText="暂无 DNS"
          onChange={(containerDns) => onFormChange({ ...form, containerDns })}
        />
        <StringListRowsField
          label="DNS Search"
          rows={form.containerDnsSearch}
          valuePlaceholder="home.arpa"
          addLabel="添加搜索域"
          emptyText="暂无 DNS Search"
          onChange={(containerDnsSearch) => onFormChange({ ...form, containerDnsSearch })}
        />
        <ExtraHostRowsField
          rows={form.containerExtraHosts}
          onChange={(containerExtraHosts) => onFormChange({ ...form, containerExtraHosts })}
        />
      </div>
    </div>
  );
}

function ContainerStorageFields({
  form,
  selectedHost,
  onFormChange,
}: {
  form: DockerFormState;
  selectedHost: Host | null;
  onFormChange: (form: DockerFormState) => void;
}) {
  const canBrowseHostDirectories = Boolean(
    selectedHost?.capabilities.some((capability) => capability.name === "files_read"),
  );

  return (
    <div className="space-y-4">
      <BindMountRowsField
        rows={form.containerBinds}
        hostId={form.hostId}
        canBrowseHostDirectories={canBrowseHostDirectories}
        onChange={(containerBinds) => onFormChange({ ...form, containerBinds })}
      />
      <div className="grid gap-4 lg:grid-cols-2">
        <AnonymousVolumeRowsField
          rows={form.containerVolumes}
          onChange={(containerVolumes) => onFormChange({ ...form, containerVolumes })}
        />
        <TmpfsRowsField
          rows={form.containerTmpfs}
          onChange={(containerTmpfs) => onFormChange({ ...form, containerTmpfs })}
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
        <StringListRowsField
          label="Cap Add"
          rows={form.containerCapAdd}
          valuePlaceholder="NET_ADMIN"
          addLabel="添加 Capability"
          emptyText="暂无 Cap Add"
          onChange={(containerCapAdd) => onFormChange({ ...form, containerCapAdd })}
        />
        <StringListRowsField
          label="Cap Drop"
          rows={form.containerCapDrop}
          valuePlaceholder="ALL"
          addLabel="添加 Capability"
          emptyText="暂无 Cap Drop"
          onChange={(containerCapDrop) => onFormChange({ ...form, containerCapDrop })}
        />
        <DeviceRowsField
          rows={form.containerDevices}
          onChange={(containerDevices) => onFormChange({ ...form, containerDevices })}
        />
        <KeyValueRowsField
          label="日志选项"
          rows={form.containerLogOptions}
          keyPlaceholder="max-size"
          valuePlaceholder="10m"
          addLabel="添加选项"
          emptyText="暂无日志选项"
          onChange={(containerLogOptions) => onFormChange({ ...form, containerLogOptions })}
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

let formRowId = 0;

function nextFormRowId() {
  formRowId += 1;
  return `container-form-row-${formRowId}`;
}

function emptyPortMappingRow(): PortMappingRow {
  return {
    id: nextFormRowId(),
    hostIp: "",
    hostPort: "",
    containerPort: "",
    protocol: "tcp",
  };
}

function emptyBindMountRow(): BindMountRow {
  return {
    id: nextFormRowId(),
    source: "",
    target: "",
    mode: "default",
  };
}

function emptyAnonymousVolumeRow(): AnonymousVolumeRow {
  return {
    id: nextFormRowId(),
    target: "",
  };
}

function emptyTmpfsMountRow(): TmpfsMountRow {
  return {
    id: nextFormRowId(),
    target: "",
    options: "",
  };
}

function emptyExtraHostRow(): ExtraHostRow {
  return {
    id: nextFormRowId(),
    hostname: "",
    address: "",
  };
}

function emptyDeviceMappingRow(): DeviceMappingRow {
  return {
    id: nextFormRowId(),
    hostPath: "",
    containerPath: "",
    permissions: "default",
  };
}

function PortMappingsField({
  rows,
  onChange,
}: {
  rows: PortMappingRow[];
  onChange: (rows: PortMappingRow[]) => void;
}) {
  const updateRow = (id: string, patch: Partial<PortMappingRow>) => {
    onChange(rows.map((row) => (row.id === id ? { ...row, ...patch } : row)));
  };

  return (
    <div className="space-y-3 rounded-md border bg-muted/10 p-3">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <span className="text-sm font-medium">端口映射</span>
        <Button
          type="button"
          variant="outline"
          size="sm"
          onClick={() => onChange([...rows, emptyPortMappingRow()])}
        >
          <Plus className="size-4" aria-hidden="true" />
          添加端口
        </Button>
      </div>

      {rows.length === 0 ? (
        <div className="rounded-md border border-dashed bg-background px-3 py-4 text-center text-sm text-muted-foreground">
          暂无端口映射
        </div>
      ) : (
        <div className="space-y-2">
          <div className="hidden grid-cols-[minmax(0,1fr)_minmax(0,0.8fr)_minmax(0,1fr)_6rem_2.25rem] gap-2 px-1 text-xs font-medium text-muted-foreground md:grid">
            <span>监听地址</span>
            <span>宿主机端口</span>
            <span>容器端口</span>
            <span>协议</span>
            <span className="sr-only">操作</span>
          </div>
          {rows.map((row) => (
            <div
              key={row.id}
              className="grid gap-2 md:grid-cols-[minmax(0,1fr)_minmax(0,0.8fr)_minmax(0,1fr)_6rem_2.25rem]"
            >
              <input
                value={row.hostIp}
                onChange={(event) => updateRow(row.id, { hostIp: event.target.value })}
                placeholder="127.0.0.1"
                aria-label="监听地址"
                className="h-9 min-w-0 rounded-md border bg-background px-3 text-sm outline-none ring-offset-background focus-visible:ring-2 focus-visible:ring-ring"
              />
              <input
                value={row.hostPort}
                onChange={(event) => updateRow(row.id, { hostPort: event.target.value })}
                placeholder="8080"
                aria-label="宿主机端口"
                className="h-9 min-w-0 rounded-md border bg-background px-3 text-sm outline-none ring-offset-background focus-visible:ring-2 focus-visible:ring-ring"
              />
              <input
                value={row.containerPort}
                onChange={(event) => updateRow(row.id, { containerPort: event.target.value })}
                placeholder="80"
                aria-label="容器端口"
                className="h-9 min-w-0 rounded-md border bg-background px-3 text-sm outline-none ring-offset-background focus-visible:ring-2 focus-visible:ring-ring"
              />
              <Select
                value={row.protocol}
                onValueChange={(protocol) => updateRow(row.id, { protocol })}
                aria-label="协议"
                options={[
                  { value: "tcp", label: "tcp" },
                  { value: "udp", label: "udp" },
                ]}
              />
              <Button
                type="button"
                variant="outline"
                size="icon"
                aria-label="删除端口映射"
                onClick={() => onChange(rows.filter((item) => item.id !== row.id))}
              >
                <Trash2 className="size-4" aria-hidden="true" />
              </Button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function BindMountRowsField({
  rows,
  hostId,
  canBrowseHostDirectories,
  onChange,
}: {
  rows: BindMountRow[];
  hostId: string;
  canBrowseHostDirectories: boolean;
  onChange: (rows: BindMountRow[]) => void;
}) {
  const [pickerRowId, setPickerRowId] = useState<string | null>(null);
  const pickerRow = rows.find((row) => row.id === pickerRowId) ?? null;
  const updateRow = (id: string, patch: Partial<BindMountRow>) => {
    onChange(rows.map((row) => (row.id === id ? { ...row, ...patch } : row)));
  };

  return (
    <div className="space-y-3 rounded-md border bg-muted/10 p-3">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <p className="text-sm font-medium">Bind / 命名卷</p>
          <p className="mt-1 text-xs text-muted-foreground">
            来源可以是宿主机目录或 Docker 命名卷。
          </p>
        </div>
        <Button
          type="button"
          variant="outline"
          size="sm"
          onClick={() => onChange([...rows, emptyBindMountRow()])}
        >
          <Plus className="size-4" aria-hidden="true" />
          添加挂载
        </Button>
      </div>

      {rows.length === 0 ? (
        <div className="rounded-md border border-dashed bg-background px-3 py-4 text-center text-sm text-muted-foreground">
          暂无挂载
        </div>
      ) : (
        <div className="space-y-2">
          <div className="hidden grid-cols-[minmax(0,1.4fr)_minmax(0,1fr)_7rem_2.25rem] gap-2 px-1 text-xs font-medium text-muted-foreground sm:grid">
            <span>来源</span>
            <span>容器路径</span>
            <span>模式</span>
            <span className="sr-only">操作</span>
          </div>
          {rows.map((row) => (
            <div
              key={row.id}
              className="grid gap-2 sm:grid-cols-[minmax(0,1.4fr)_minmax(0,1fr)_7rem_2.25rem]"
            >
              <div className="flex min-w-0 gap-2">
                <input
                  value={row.source}
                  onChange={(event) => updateRow(row.id, { source: event.target.value })}
                  placeholder="/home/doro/www 或 app-data"
                  aria-label="挂载来源"
                  className="h-9 min-w-0 flex-1 rounded-md border bg-background px-3 text-sm outline-none ring-offset-background focus-visible:ring-2 focus-visible:ring-ring"
                />
                <Button
                  type="button"
                  variant="outline"
                  size="icon"
                  aria-label="选择宿主机目录"
                  disabled={!hostId || !canBrowseHostDirectories}
                  onClick={() => setPickerRowId(row.id)}
                >
                  <FolderOpen className="size-4" aria-hidden="true" />
                </Button>
              </div>
              <input
                value={row.target}
                onChange={(event) => updateRow(row.id, { target: event.target.value })}
                placeholder="/usr/share/nginx/html"
                aria-label="容器内路径"
                className="h-9 min-w-0 rounded-md border bg-background px-3 text-sm outline-none ring-offset-background focus-visible:ring-2 focus-visible:ring-ring"
              />
              <Select
                value={row.mode}
                onValueChange={(mode) => updateRow(row.id, { mode })}
                aria-label="挂载模式"
                options={[
                  { value: "default", label: "默认" },
                  { value: "rw", label: "读写" },
                  { value: "ro", label: "只读" },
                ]}
              />
              <Button
                type="button"
                variant="outline"
                size="icon"
                aria-label="删除挂载"
                onClick={() => onChange(rows.filter((item) => item.id !== row.id))}
              >
                <Trash2 className="size-4" aria-hidden="true" />
              </Button>
            </div>
          ))}
        </div>
      )}

      {!canBrowseHostDirectories && hostId ? (
        <p className="text-xs text-muted-foreground">
          当前 Agent 未声明 files_read capability，宿主机目录可手动输入。
        </p>
      ) : null}

      <HostDirectoryPickerDialog
        open={Boolean(pickerRow)}
        hostId={hostId}
        initialPath={pickerRow?.source.startsWith("/") ? pickerRow.source : undefined}
        onOpenChange={(open) => {
          if (!open) {
            setPickerRowId(null);
          }
        }}
        onSelect={(source) => {
          if (pickerRow) {
            updateRow(pickerRow.id, { source });
          }
        }}
      />
    </div>
  );
}

function AnonymousVolumeRowsField({
  rows,
  onChange,
}: {
  rows: AnonymousVolumeRow[];
  onChange: (rows: AnonymousVolumeRow[]) => void;
}) {
  const updateRow = (id: string, patch: Partial<AnonymousVolumeRow>) => {
    onChange(rows.map((row) => (row.id === id ? { ...row, ...patch } : row)));
  };

  return (
    <div className="space-y-3 rounded-md border bg-muted/10 p-3">
      <div className="flex items-center justify-between gap-3">
        <span className="text-sm font-medium">匿名卷</span>
        <Button
          type="button"
          variant="outline"
          size="sm"
          onClick={() => onChange([...rows, emptyAnonymousVolumeRow()])}
        >
          <Plus className="size-4" aria-hidden="true" />
          添加路径
        </Button>
      </div>

      {rows.length === 0 ? (
        <div className="rounded-md border border-dashed bg-background px-3 py-4 text-center text-sm text-muted-foreground">
          暂无匿名卷
        </div>
      ) : (
        <div className="space-y-2">
          {rows.map((row) => (
            <div key={row.id} className="grid gap-2 sm:grid-cols-[minmax(0,1fr)_2.25rem]">
              <input
                value={row.target}
                onChange={(event) => updateRow(row.id, { target: event.target.value })}
                placeholder="/cache"
                aria-label="匿名卷容器路径"
                className="h-9 min-w-0 rounded-md border bg-background px-3 text-sm outline-none ring-offset-background focus-visible:ring-2 focus-visible:ring-ring"
              />
              <Button
                type="button"
                variant="outline"
                size="icon"
                aria-label="删除匿名卷"
                onClick={() => onChange(rows.filter((item) => item.id !== row.id))}
              >
                <Trash2 className="size-4" aria-hidden="true" />
              </Button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function TmpfsRowsField({
  rows,
  onChange,
}: {
  rows: TmpfsMountRow[];
  onChange: (rows: TmpfsMountRow[]) => void;
}) {
  const updateRow = (id: string, patch: Partial<TmpfsMountRow>) => {
    onChange(rows.map((row) => (row.id === id ? { ...row, ...patch } : row)));
  };

  return (
    <div className="space-y-3 rounded-md border bg-muted/10 p-3">
      <div className="flex items-center justify-between gap-3">
        <span className="text-sm font-medium">tmpfs</span>
        <Button
          type="button"
          variant="outline"
          size="sm"
          onClick={() => onChange([...rows, emptyTmpfsMountRow()])}
        >
          <Plus className="size-4" aria-hidden="true" />
          添加 tmpfs
        </Button>
      </div>

      {rows.length === 0 ? (
        <div className="rounded-md border border-dashed bg-background px-3 py-4 text-center text-sm text-muted-foreground">
          暂无 tmpfs
        </div>
      ) : (
        <div className="space-y-2">
          {rows.map((row) => (
            <div key={row.id} className="grid gap-2 sm:grid-cols-[minmax(0,1fr)_minmax(0,1fr)_2.25rem]">
              <input
                value={row.target}
                onChange={(event) => updateRow(row.id, { target: event.target.value })}
                placeholder="/run"
                aria-label="tmpfs 路径"
                className="h-9 min-w-0 rounded-md border bg-background px-3 text-sm outline-none ring-offset-background focus-visible:ring-2 focus-visible:ring-ring"
              />
              <input
                value={row.options}
                onChange={(event) => updateRow(row.id, { options: event.target.value })}
                placeholder="rw,size=64m"
                aria-label="tmpfs 选项"
                className="h-9 min-w-0 rounded-md border bg-background px-3 text-sm outline-none ring-offset-background focus-visible:ring-2 focus-visible:ring-ring"
              />
              <Button
                type="button"
                variant="outline"
                size="icon"
                aria-label="删除 tmpfs"
                onClick={() => onChange(rows.filter((item) => item.id !== row.id))}
              >
                <Trash2 className="size-4" aria-hidden="true" />
              </Button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function ExtraHostRowsField({
  rows,
  onChange,
}: {
  rows: ExtraHostRow[];
  onChange: (rows: ExtraHostRow[]) => void;
}) {
  const updateRow = (id: string, patch: Partial<ExtraHostRow>) => {
    onChange(rows.map((row) => (row.id === id ? { ...row, ...patch } : row)));
  };

  return (
    <div className="space-y-3 rounded-md border bg-muted/10 p-3">
      <div className="flex items-center justify-between gap-3">
        <span className="text-sm font-medium">Extra Hosts</span>
        <Button
          type="button"
          variant="outline"
          size="sm"
          onClick={() => onChange([...rows, emptyExtraHostRow()])}
        >
          <Plus className="size-4" aria-hidden="true" />
          添加 Host
        </Button>
      </div>

      {rows.length === 0 ? (
        <div className="rounded-md border border-dashed bg-background px-3 py-4 text-center text-sm text-muted-foreground">
          暂无 Extra Hosts
        </div>
      ) : (
        <div className="space-y-2">
          {rows.map((row) => (
            <div key={row.id} className="grid gap-2 sm:grid-cols-[minmax(0,1fr)_minmax(0,1fr)_2.25rem]">
              <input
                value={row.hostname}
                onChange={(event) => updateRow(row.id, { hostname: event.target.value })}
                placeholder="host.docker.internal"
                aria-label="Host 名称"
                className="h-9 min-w-0 rounded-md border bg-background px-3 text-sm outline-none ring-offset-background focus-visible:ring-2 focus-visible:ring-ring"
              />
              <input
                value={row.address}
                onChange={(event) => updateRow(row.id, { address: event.target.value })}
                placeholder="host-gateway"
                aria-label="Host 地址"
                className="h-9 min-w-0 rounded-md border bg-background px-3 text-sm outline-none ring-offset-background focus-visible:ring-2 focus-visible:ring-ring"
              />
              <Button
                type="button"
                variant="outline"
                size="icon"
                aria-label="删除 Extra Host"
                onClick={() => onChange(rows.filter((item) => item.id !== row.id))}
              >
                <Trash2 className="size-4" aria-hidden="true" />
              </Button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function DeviceRowsField({
  rows,
  onChange,
}: {
  rows: DeviceMappingRow[];
  onChange: (rows: DeviceMappingRow[]) => void;
}) {
  const updateRow = (id: string, patch: Partial<DeviceMappingRow>) => {
    onChange(rows.map((row) => (row.id === id ? { ...row, ...patch } : row)));
  };

  return (
    <div className="space-y-3 rounded-md border bg-muted/10 p-3">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <span className="text-sm font-medium">设备</span>
        <Button
          type="button"
          variant="outline"
          size="sm"
          onClick={() => onChange([...rows, emptyDeviceMappingRow()])}
        >
          <Plus className="size-4" aria-hidden="true" />
          添加设备
        </Button>
      </div>

      {rows.length === 0 ? (
        <div className="rounded-md border border-dashed bg-background px-3 py-4 text-center text-sm text-muted-foreground">
          暂无设备
        </div>
      ) : (
        <div className="space-y-2">
          <div className="hidden grid-cols-[minmax(0,1fr)_minmax(0,1fr)_7rem_2.25rem] gap-2 px-1 text-xs font-medium text-muted-foreground md:grid">
            <span>宿主机路径</span>
            <span>容器路径</span>
            <span>权限</span>
            <span className="sr-only">操作</span>
          </div>
          {rows.map((row) => (
            <div
              key={row.id}
              className="grid gap-2 md:grid-cols-[minmax(0,1fr)_minmax(0,1fr)_7rem_2.25rem]"
            >
              <input
                value={row.hostPath}
                onChange={(event) => updateRow(row.id, { hostPath: event.target.value })}
                placeholder="/dev/fuse"
                aria-label="设备宿主机路径"
                className="h-9 min-w-0 rounded-md border bg-background px-3 text-sm outline-none ring-offset-background focus-visible:ring-2 focus-visible:ring-ring"
              />
              <input
                value={row.containerPath}
                onChange={(event) => updateRow(row.id, { containerPath: event.target.value })}
                placeholder="/dev/fuse"
                aria-label="设备容器路径"
                className="h-9 min-w-0 rounded-md border bg-background px-3 text-sm outline-none ring-offset-background focus-visible:ring-2 focus-visible:ring-ring"
              />
              <Select
                value={row.permissions}
                onValueChange={(permissions) => updateRow(row.id, { permissions })}
                aria-label="设备权限"
                options={[
                  { value: "default", label: "默认" },
                  { value: "rwm", label: "rwm" },
                  { value: "rw", label: "rw" },
                  { value: "r", label: "r" },
                ]}
              />
              <Button
                type="button"
                variant="outline"
                size="icon"
                aria-label="删除设备"
                onClick={() => onChange(rows.filter((item) => item.id !== row.id))}
              >
                <Trash2 className="size-4" aria-hidden="true" />
              </Button>
            </div>
          ))}
        </div>
      )}
    </div>
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
  type = "text",
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  required?: boolean;
  disabled?: boolean;
  placeholder?: string;
  type?: string;
}) {
  return (
    <Field label={label}>
      <input
        type={type}
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
        statusLabel: item.architecture ?? "unknown",
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
      env: keyValueRowsToLines(form.containerEnv),
      labels: keyValueRowsToObject(form.containerLabels),
      network_mode: containerNetworkMode(form),
      network_name: form.networkMode === "custom" ? optionalText(form.networkName) : null,
      aliases: stringListRowsToLines(form.containerAliases),
      ipv4_address: optionalText(form.ipv4Address),
      mac_address: optionalText(form.macAddress),
      ports: portRowsToBindings(form.containerPorts),
      dns: stringListRowsToLines(form.containerDns),
      dns_search: stringListRowsToLines(form.containerDnsSearch),
      extra_hosts: extraHostRowsToLines(form.containerExtraHosts),
      binds: bindMountRowsToLines(form.containerBinds),
      volumes: anonymousVolumeRowsToLines(form.containerVolumes),
      tmpfs: tmpfsRowsToLines(form.containerTmpfs),
      shm_size: optionalText(form.shmSize),
      restart_policy: containerRestartPolicy(form.restartPolicy),
      restart_max_retries: optionalInteger(form.restartMaxRetries),
      auto_remove: form.autoRemove,
      privileged: form.privileged,
      init: form.init,
      tty: form.tty,
      open_stdin: form.openStdin,
      read_only_rootfs: form.readOnlyRootfs,
      cap_add: stringListRowsToLines(form.containerCapAdd),
      cap_drop: stringListRowsToLines(form.containerCapDrop),
      devices: deviceRowsToDevices(form.containerDevices),
      memory: optionalText(form.memory),
      memory_swap: optionalText(form.memorySwap),
      cpus: optionalText(form.cpus),
      cpu_shares: optionalInteger(form.cpuShares),
      cpuset_cpus: optionalText(form.cpusetCpus),
      pids_limit: optionalInteger(form.pidsLimit),
      healthcheck: containerHealthcheck(form),
      log_driver: optionalText(form.logDriver),
      log_options: keyValueRowsToObject(form.containerLogOptions),
      reason: form.requiresApproval ? reason : null,
    });
  }
  if (dialog.kind === "image-pull") {
    return pullDockerImage({
      host_id: form.hostId,
      reference: form.reference.trim(),
      tag: optionalText(form.tag),
      platform: optionalText(form.platform),
      reason: null,
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
      labels: keyValueRowsToObject(form.operationLabels),
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
      driver_opts: keyValueRowsToObject(form.driverOptionRows),
      labels: keyValueRowsToObject(form.operationLabels),
      reason,
    });
  }

  const request = {
    host_id: form.hostId,
    name: form.name.trim(),
    compose_yaml: form.composeYaml,
    env_file: optionalText(form.envFile),
    reason: null,
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

function dialogSubmitMessage(kind: DialogKind, form: DockerFormState) {
  if (kind === "image-pull") {
    return "已创建 Docker 镜像拉取任务，Agent 正在执行该操作。";
  }
  if (kind === "compose-edit") {
    return "已创建 Docker Compose 保存任务，Agent 正在执行该操作。";
  }
  if (kind === "container-create" && !form.requiresApproval) {
    return "已创建 Docker 容器任务，Agent 正在执行该操作。";
  }
  return "已创建 Docker 审批任务，批准后 Agent 会执行该操作。";
}

function dialogSubmitLabel(kind?: DialogKind, row?: DockerRow) {
  if (kind === "image-pull") {
    return "开始拉取";
  }
  if (kind === "compose-edit") {
    return row ? "保存项目" : "创建项目";
  }
  return "提交审批";
}

function composeActionMessage(action: "up" | "down" | "restart" | "pull" | "delete") {
  if (action === "up" || action === "pull") {
    return "已创建 Docker 任务，Agent 正在执行该操作。";
  }
  return "已创建 Docker 审批任务，批准后 Agent 会执行该操作。";
}

function registrySourceLabel(source: string) {
  if (source === "inline") {
    return "Doro 管理的内联凭证";
  }
  if (source === "removed") {
    return "已删除";
  }
  return "Docker credential helper 管理";
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
  if (kind === "image-pull") {
    return "拉取镜像会直接创建任务并由目标 Agent 执行，Agent 会使用本机 Docker registry 配置。";
  }
  if (kind === "compose-edit") {
    return "Compose 文件会写入 Agent 配置的受控目录，并直接创建任务由目标 Agent 执行。";
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

function splitWords(value: string) {
  return value.trim() ? value.trim().split(/\s+/) : [];
}

function keyValueRowsToLines(rows: KeyValueRow[]) {
  return rows.flatMap((row) => {
    const key = row.key.trim();
    if (!key) {
      return [];
    }
    return [`${key}=${row.value}`];
  });
}

function keyValueRowsToObject(rows: KeyValueRow[]) {
  return Object.fromEntries(
    rows.flatMap((row) => {
      const key = row.key.trim();
      if (!key) {
        return [];
      }
      return [[key, row.value]];
    }),
  );
}

function stringListRowsToLines(rows: StringListRow[]) {
  return rows
    .map((row) => row.value.trim())
    .filter(Boolean);
}

function portRowsToBindings(rows: PortMappingRow[]) {
  return rows.flatMap((row) => {
    const containerPort = row.containerPort.trim();
    if (!containerPort) {
      return [];
    }
    return [
      {
        container_port: containerPort,
        protocol: optionalText(row.protocol),
        host_ip: optionalText(row.hostIp),
        host_port: optionalText(row.hostPort),
      },
    ];
  });
}

function bindMountRowsToLines(rows: BindMountRow[]) {
  return rows.flatMap((row) => {
    const source = row.source.trim();
    const target = row.target.trim();
    if (!source || !target) {
      return [];
    }
    const mode = row.mode === "default" ? "" : row.mode.trim();
    return [[source, target, mode].filter(Boolean).join(":")];
  });
}

function anonymousVolumeRowsToLines(rows: AnonymousVolumeRow[]) {
  return rows
    .map((row) => row.target.trim())
    .filter(Boolean);
}

function tmpfsRowsToLines(rows: TmpfsMountRow[]) {
  return rows.flatMap((row) => {
    const target = row.target.trim();
    if (!target) {
      return [];
    }
    const options = row.options.trim();
    return [options ? `${target}:${options}` : target];
  });
}

function extraHostRowsToLines(rows: ExtraHostRow[]) {
  return rows.flatMap((row) => {
    const hostname = row.hostname.trim();
    const address = row.address.trim();
    if (!hostname || !address) {
      return [];
    }
    return [`${hostname}:${address}`];
  });
}

function deviceRowsToDevices(rows: DeviceMappingRow[]) {
  return rows.flatMap((row) => {
    const hostPath = row.hostPath.trim();
    if (!hostPath) {
      return [];
    }
    return [
      {
        host_path: hostPath,
        container_path: optionalText(row.containerPath),
        permissions: row.permissions === "default" ? null : optionalText(row.permissions),
      },
    ];
  });
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
