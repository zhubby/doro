"use client";

import {
  Eye,
  Filter,
  Play,
  Plus,
  RefreshCw,
  RotateCw,
  Search,
  Square,
  Trash2,
  Camera,
} from "lucide-react";
import {
  type FormEvent,
  type HTMLAttributes,
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
import { useToastMessage } from "@/components/ui/use-toast-message";
import {
  createVirtualMachine,
  createVirtualMachineSnapshot,
  getHosts,
  getVirtualMachineConsole,
  getVirtualMachineImages,
  getVirtualMachineSnapshots,
  getVirtualMachineTemplates,
  refreshVirtualMachines,
  virtualMachineAction,
} from "@/lib/control-plane-api";
import type {
  Host,
  VirtualMachine,
  VirtualMachineActionResponse,
  VirtualMachineImage,
  VirtualMachineSnapshot,
  VirtualMachineTemplate,
} from "@/types/api";
import type { ResourceColumn, ResourceStatus } from "@/types/dashboard";

type VirtualMachineKind = "instances" | "images" | "snapshots" | "templates";

type VirtualMachinesPageProps = {
  kind: VirtualMachineKind;
};

type VmRow = {
  id: string;
  hostId: string;
  hostLabel: string;
  name: string;
  secondary: string;
  status: ResourceStatus;
  detailA: string;
  detailB: string;
  detailC: string;
  raw: VirtualMachine | VirtualMachineImage | VirtualMachineSnapshot | VirtualMachineTemplate;
};

type DialogKind = "create" | "snapshot" | "console";

type ActiveDialog = {
  kind: DialogKind;
  row?: VmRow;
  console?: Record<string, unknown>;
} | null;

type VmFormState = {
  hostId: string;
  name: string;
  imageId: string;
  templateId: string;
  cpuCores: string;
  memoryMib: string;
  diskGb: string;
  networkMode: "user_nat" | "bridge_tap";
  bridge: string;
  portForwards: string;
  snapshotName: string;
  snapshotDescription: string;
  reason: string;
};

const emptyForm: VmFormState = {
  hostId: "",
  name: "",
  imageId: "",
  templateId: "",
  cpuCores: "2",
  memoryMib: "2048",
  diskGb: "20",
  networkMode: "user_nat",
  bridge: "",
  portForwards: "",
  snapshotName: "",
  snapshotDescription: "",
  reason: "",
};

const kindMeta: Record<VirtualMachineKind, { title: string; description: string; empty: string }> = {
  instances: {
    title: "实例",
    description: "实时刷新 QEMU 虚拟机实例，并通过审批任务执行生命周期操作。",
    empty: "暂无虚拟机实例。启用 Agent 的 QEMU 能力后，刷新会显示宿主机上报的虚拟机。",
  },
  images: {
    title: "镜像",
    description: "查看在线 VM Agent 本地镜像缓存，创建实例时会复制镜像为托管启动盘。",
    empty: "暂无 QEMU 镜像。请在 Agent 的 vm_image_dir 放入 qcow2 或 img 文件。",
  },
  snapshots: {
    title: "快照",
    description: "选择虚拟机查看 stopped 状态下创建的快照元数据。",
    empty: "暂无快照记录。",
  },
  templates: {
    title: "模板",
    description: "查看虚拟机创建表单可套用的资源模板；MVP 暂不提供模板编辑。",
    empty: "暂无虚拟机模板。",
  },
};

const columns: ResourceColumn<VmRow>[] = [
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
    label: "配置",
    width: "17%",
    render: (row) => <TruncatedText value={row.detailA} />,
  },
  {
    key: "detailB",
    label: "资源",
    width: "15%",
    render: (row) => <TruncatedText value={row.detailB} />,
  },
  {
    key: "detailC",
    label: "附加信息",
    width: "18%",
    render: (row) => <TruncatedText value={row.detailC} />,
  },
];

export function VirtualMachinesPage({ kind }: VirtualMachinesPageProps) {
  const [hosts, setHosts] = useState<Host[]>([]);
  const [machines, setMachines] = useState<VirtualMachine[]>([]);
  const [images, setImages] = useState<VirtualMachineImage[]>([]);
  const [templates, setTemplates] = useState<VirtualMachineTemplate[]>([]);
  const [snapshots, setSnapshots] = useState<VirtualMachineSnapshot[]>([]);
  const [selectedVmId, setSelectedVmId] = useState("");
  const [query, setQuery] = useState("");
  const [hostFilter, setHostFilter] = useState("all");
  const [loading, setLoading] = useState(true);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [apiError, setApiError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [activeDialog, setActiveDialog] = useState<ActiveDialog>(null);
  const [form, setForm] = useState<VmFormState>(emptyForm);
  const meta = kindMeta[kind];

  const vmHosts = useMemo(
    () =>
      hosts.filter(
        (host) =>
          host.status === "online" &&
          host.capabilities.some(
            (capability) => capability.name === "virtual_machines_manage",
          ),
      ),
    [hosts],
  );
  const hostNames = useMemo(
    () => new Map(hosts.map((host) => [host.id, hostLabel(host)])),
    [hosts],
  );
  const selectedHostId = hostFilter === "all" ? undefined : hostFilter;

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

    const [machinesResult, imagesResult, templatesResult] = await Promise.all([
      kind === "instances" || kind === "snapshots"
        ? refreshVirtualMachines()
        : Promise.resolve({ data: null, error: null }),
      kind === "instances" || kind === "images"
        ? getVirtualMachineImages(selectedHostId)
        : Promise.resolve({ data: null, error: null }),
      kind === "instances" || kind === "templates"
        ? getVirtualMachineTemplates()
        : Promise.resolve({ data: null, error: null }),
    ]);

    if (machinesResult.data) {
      setMachines(machinesResult.data.items);
      if (kind === "snapshots" && !selectedVmId && machinesResult.data.items[0]) {
        setSelectedVmId(machinesResult.data.items[0].id);
      }
    }
    if (imagesResult.data) {
      setImages(imagesResult.data.items);
    }
    if (templatesResult.data) {
      setTemplates(templatesResult.data.items);
    }

    const snapshotVmId =
      kind === "snapshots"
        ? selectedVmId || machinesResult.data?.items[0]?.id || ""
        : "";
    if (snapshotVmId) {
      const snapshotResult = await getVirtualMachineSnapshots(snapshotVmId);
      if (snapshotResult.data) {
        setSnapshots(snapshotResult.data.items);
      } else {
        setApiError(snapshotResult.error);
      }
    }

    const error =
      hostsResult.error ??
      machinesResult.error ??
      imagesResult.error ??
      templatesResult.error ??
      null;
    setApiError(error);
    setLoading(false);
  }, [kind, selectedHostId, selectedVmId]);

  useEffect(() => {
    void loadData();
  }, [loadData]);

  useToastMessage(apiError, {
    id: `virtual-machines-${kind}-api-error`,
    kind: "error",
    prefix: "虚拟机管理暂不可用：",
  });
  useToastMessage(notice, { id: `virtual-machines-${kind}-notice`, kind: "success" });

  const rows = useMemo(() => {
    if (kind === "instances") {
      return machines
        .filter((machine) => hostFilter === "all" || machine.host_id === hostFilter)
        .map((machine) => machineRow(machine, hostNames));
    }
    if (kind === "images") {
      return images.map((image) => imageRow(image, hostNames));
    }
    if (kind === "snapshots") {
      const selectedVm = machines.find((machine) => machine.id === selectedVmId);
      return snapshots.map((snapshot) => snapshotRow(snapshot, selectedVm, hostNames));
    }
    return templates.map(templateRow);
  }, [hostFilter, hostNames, images, kind, machines, selectedVmId, snapshots, templates]);

  const filteredRows = useMemo(() => {
    const normalizedQuery = query.trim().toLowerCase();
    return rows.filter((row) => {
      if (!normalizedQuery) {
        return true;
      }
      return [row.name, row.secondary, row.hostLabel, row.detailA, row.detailB, row.detailC]
        .join(" ")
        .toLowerCase()
        .includes(normalizedQuery);
    });
  }, [query, rows]);

  function openCreateDialog() {
    const hostId = selectedHostId ?? vmHosts[0]?.id ?? "";
    const firstTemplate = templates[0];
    const hostImages = images.filter((image) => !image.host_id || image.host_id === hostId);
    const templateImage = hostImages.find((image) => image.id === firstTemplate?.image_id);
    const firstImage = templateImage ?? hostImages[0] ?? images[0];
    setForm({
      ...emptyForm,
      hostId,
      imageId: firstImage?.id ?? "",
      templateId: firstTemplate?.id ?? "",
      cpuCores: firstTemplate ? String(firstTemplate.cpu_cores) : "2",
      memoryMib: firstTemplate ? String(firstTemplate.memory_mib) : "2048",
      diskGb: firstTemplate ? String(firstTemplate.disk_gb) : "20",
      reason: "operator requested qemu virtual machine create",
    });
    setActiveDialog({ kind: "create" });
  }

  function openSnapshotDialog(row?: VmRow) {
    const machine = row?.raw as VirtualMachine | undefined;
    setForm({
      ...emptyForm,
      snapshotName: machine ? `${machine.name}-snapshot` : "",
      snapshotDescription: "",
      reason: "operator requested qemu virtual machine snapshot",
    });
    setActiveDialog({ kind: "snapshot", row });
  }

  async function openConsoleDialog(row: VmRow) {
    setBusyId(row.id);
    const result = await getVirtualMachineConsole(row.id);
    if (result.data) {
      setActiveDialog({
        kind: "console",
        row,
        console: result.data.item as Record<string, unknown>,
      });
      setApiError(null);
    } else {
      setApiError(result.error);
    }
    setBusyId(null);
  }

  async function submitDialog(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!activeDialog) {
      return;
    }
    setBusyId(activeDialog.row?.id ?? activeDialog.kind);
    setNotice(null);
    const result =
      activeDialog.kind === "create"
        ? await createVirtualMachine({
            host_id: form.hostId,
            name: form.name.trim(),
            image_id: form.imageId,
            cpu_cores: Number(form.cpuCores),
            memory_mib: Number(form.memoryMib),
            disk_gb: Number(form.diskGb),
            networks: [
              {
                mode: form.networkMode,
                bridge: form.networkMode === "bridge_tap" ? form.bridge.trim() || null : null,
                mac_address: null,
                port_forwards: parsePortForwards(form.portForwards),
              },
            ],
            cloud_init: {},
            reason: form.reason.trim() || null,
          })
        : activeDialog.kind === "snapshot" && activeDialog.row
          ? await createVirtualMachineSnapshot(activeDialog.row.id, {
              name: form.snapshotName.trim(),
              description: form.snapshotDescription.trim() || null,
              reason: form.reason.trim() || null,
            })
          : { data: null, error: null };

    handleActionResult(result);
    if (result.data) {
      setActiveDialog(null);
      await loadData();
    }
    setBusyId(null);
  }

  async function runInstanceAction(
    row: VmRow,
    action: "start" | "stop" | "restart" | "delete",
  ) {
    if (action === "delete" && !window.confirm(`删除虚拟机 ${row.name}？`)) {
      return;
    }
    setBusyId(row.id);
    const result = await virtualMachineAction(row.id, action, {
      reason: `operator requested qemu virtual machine ${action}`,
    });
    handleActionResult(result);
    await loadData();
    setBusyId(null);
  }

  function handleActionResult(
    result: { data: VirtualMachineActionResponse | null; error: string | null },
  ) {
    if (result.data) {
      setNotice(`已创建虚拟机审批任务，批准后 Agent 会执行该操作。任务：${shortId(result.data.task.id)}`);
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
            <>
              {kind === "instances" ? (
                <Button onClick={openCreateDialog} disabled={vmHosts.length === 0}>
                  <Plus className="size-4" aria-hidden="true" />
                  创建虚拟机
                </Button>
              ) : null}
              {kind === "snapshots" ? (
                <Button
                  onClick={() => {
                    const machine = machines.find((item) => item.id === selectedVmId);
                    openSnapshotDialog(machine ? machineRow(machine, hostNames) : undefined);
                  }}
                  disabled={!selectedVmId}
                >
                  <Camera className="size-4" aria-hidden="true" />
                  创建快照
                </Button>
              ) : null}
            </>
          }
          right={
            <div className="flex w-full flex-col gap-2 sm:w-auto sm:flex-row">
              {kind === "snapshots" ? (
                <Select
                  value={selectedVmId}
                  onValueChange={setSelectedVmId}
                  className="sm:w-64"
                  placeholder="选择虚拟机"
                  options={
                    machines.length === 0
                      ? [{ value: "", label: "暂无虚拟机" }]
                      : machines.map((machine) => ({
                          value: machine.id,
                          label: `${machine.name} · ${hostNames.get(machine.host_id) ?? machine.host_id}`,
                        }))
                  }
                />
              ) : null}
              <label className="relative min-w-0 sm:w-72">
                <Search
                  className="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground"
                  aria-hidden="true"
                />
                <span className="sr-only">搜索虚拟机资源</span>
                <input
                  value={query}
                  onChange={(event) => setQuery(event.target.value)}
                  placeholder="搜索名称、ID 或路径"
                  className="h-9 w-full rounded-md border bg-background pl-9 pr-3 text-sm outline-none ring-offset-background placeholder:text-muted-foreground focus-visible:ring-2 focus-visible:ring-ring"
                />
              </label>
              {(kind === "instances" || kind === "images") ? (
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
                      {vmHosts.map((host) => (
                        <DropdownMenuRadioItem key={host.id} value={host.id}>
                          {hostLabel(host)}
                        </DropdownMenuRadioItem>
                      ))}
                    </DropdownMenuRadioGroup>
                  </DropdownMenuContent>
                </DropdownMenu>
              ) : null}
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
          actionsWidth={kind === "instances" ? "23rem" : "10rem"}
          actions={kind === "instances" ? undefined : []}
          emptyText={loading ? "正在加载虚拟机数据..." : meta.empty}
          renderActions={
            kind === "instances"
              ? (row) =>
                  renderInstanceActions({
                  row,
                  busy: busyId === row.id,
                  onAction: runInstanceAction,
                  onSnapshot: () => openSnapshotDialog(row),
                  onConsole: () => void openConsoleDialog(row),
                })
              : undefined
          }
        />
      </PageSection>

      <VmDialog
        open={Boolean(activeDialog)}
        dialog={activeDialog}
        form={form}
        hosts={vmHosts}
        images={images}
        templates={templates}
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

function renderInstanceActions({
  row,
  busy,
  onAction,
  onSnapshot,
  onConsole,
}: {
  row: VmRow;
  busy: boolean;
  onAction: (row: VmRow, action: "start" | "stop" | "restart" | "delete") => void;
  onSnapshot: () => void;
  onConsole: () => void;
}) {
  return (
    <>
      {row.status === "running" ? (
        <Button variant="outline" size="sm" disabled={busy} onClick={() => onAction(row, "stop")}>
          <Square className="size-4" aria-hidden="true" />
          停止
        </Button>
      ) : (
        <Button variant="outline" size="sm" disabled={busy} onClick={() => onAction(row, "start")}>
          <Play className="size-4" aria-hidden="true" />
          启动
        </Button>
      )}
      <Button variant="outline" size="sm" disabled={busy} onClick={() => onAction(row, "restart")}>
        <RotateCw className="size-4" aria-hidden="true" />
        重启
      </Button>
      <Button variant="outline" size="sm" disabled={busy} onClick={onSnapshot}>
        <Camera className="size-4" aria-hidden="true" />
        快照
      </Button>
      <Button variant="outline" size="sm" disabled={busy} onClick={onConsole}>
        <Eye className="size-4" aria-hidden="true" />
        控制台
      </Button>
      <Button variant="outline" size="sm" disabled={busy} onClick={() => onAction(row, "delete")}>
        <Trash2 className="size-4" aria-hidden="true" />
        删除
      </Button>
    </>
  );
}

function VmDialog({
  open,
  dialog,
  form,
  hosts,
  images,
  templates,
  onOpenChange,
  onFormChange,
  onSubmit,
}: {
  open: boolean;
  dialog: ActiveDialog;
  form: VmFormState;
  hosts: Host[];
  images: VirtualMachineImage[];
  templates: VirtualMachineTemplate[];
  onOpenChange: (open: boolean) => void;
  onFormChange: (form: VmFormState) => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
}) {
  const kind = dialog?.kind;
  const title =
    kind === "create" ? "创建虚拟机" : kind === "snapshot" ? "创建快照" : "控制台";
  const description =
    kind === "create"
      ? "提交后会创建高风险审批任务，批准后 Agent 复制镜像并生成 QEMU 实例。"
      : kind === "snapshot"
        ? "MVP 仅支持 stopped 虚拟机快照，批准后 Agent 会记录快照元数据。"
        : "复制 VNC endpoint 后使用本地客户端连接。";

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className={kind === "console" ? "max-w-2xl" : "max-w-3xl"}>
        {kind === "console" ? (
          <div className="space-y-5">
            <DialogHeader>
              <DialogTitle>{title}</DialogTitle>
              <DialogDescription>{description}</DialogDescription>
            </DialogHeader>
            <pre className="max-h-96 overflow-auto rounded-md border bg-muted/30 p-4 text-xs">
              {JSON.stringify(dialog?.console ?? {}, null, 2)}
            </pre>
            <DialogFooter>
              <Button type="button" onClick={() => onOpenChange(false)}>
                关闭
              </Button>
            </DialogFooter>
          </div>
        ) : (
          <form onSubmit={onSubmit} className="space-y-5">
            <DialogHeader>
              <DialogTitle>{title}</DialogTitle>
              <DialogDescription>{description}</DialogDescription>
            </DialogHeader>

            {kind === "create" ? (
              <CreateFields
                form={form}
                hosts={hosts}
                images={images}
                templates={templates}
                onFormChange={onFormChange}
              />
            ) : null}
            {kind === "snapshot" ? (
              <SnapshotFields form={form} onFormChange={onFormChange} />
            ) : null}

            <Field label="审批原因">
              <textarea
                value={form.reason}
                onChange={(event) => onFormChange({ ...form, reason: event.target.value })}
                className="min-h-20 w-full rounded-md border bg-background px-3 py-2 text-sm outline-none ring-offset-background focus-visible:ring-2 focus-visible:ring-ring"
              />
            </Field>

            <DialogFooter>
              <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
                取消
              </Button>
              <Button type="submit" disabled={kind === "create" && (!form.hostId || !form.imageId)}>
                提交审批
              </Button>
            </DialogFooter>
          </form>
        )}
      </DialogContent>
    </Dialog>
  );
}

function CreateFields({
  form,
  hosts,
  images,
  templates,
  onFormChange,
}: {
  form: VmFormState;
  hosts: Host[];
  images: VirtualMachineImage[];
  templates: VirtualMachineTemplate[];
  onFormChange: (form: VmFormState) => void;
}) {
  const availableImages = images.filter((image) => !image.host_id || image.host_id === form.hostId);

  function applyTemplate(templateId: string) {
    const template = templates.find((item) => item.id === templateId);
    const templateImage = availableImages.find((image) => image.id === template?.image_id);
    onFormChange({
      ...form,
      templateId,
      imageId: templateImage?.id ?? form.imageId,
      cpuCores: template ? String(template.cpu_cores) : form.cpuCores,
      memoryMib: template ? String(template.memory_mib) : form.memoryMib,
      diskGb: template ? String(template.disk_gb) : form.diskGb,
    });
  }

  function changeHost(hostId: string) {
    const hostImages = images.filter((image) => !image.host_id || image.host_id === hostId);
    const imageId = hostImages.some((image) => image.id === form.imageId)
      ? form.imageId
      : hostImages[0]?.id ?? "";
    onFormChange({ ...form, hostId, imageId });
  }

  return (
    <>
      <div className="grid gap-4 sm:grid-cols-2">
        <Field label="目标 Agent">
          <Select
            required
            value={form.hostId}
            onValueChange={changeHost}
            options={
              hosts.length === 0
                ? [{ value: "", label: "暂无在线 VM Agent" }]
                : hosts.map((host) => ({ value: host.id, label: hostLabel(host) }))
            }
          />
        </Field>
        <TextField label="名称" value={form.name} onChange={(name) => onFormChange({ ...form, name })} required placeholder="home-assistant" />
        <Field label="模板">
          <Select
            value={form.templateId}
            onValueChange={applyTemplate}
            placeholder="不使用模板"
            options={[
              { value: "", label: "不使用模板" },
              ...templates.map((template) => ({ value: template.id, label: template.name })),
            ]}
          />
        </Field>
        <Field label="镜像">
          <Select
            required
            value={form.imageId}
            onValueChange={(imageId) => onFormChange({ ...form, imageId })}
            options={
              availableImages.length === 0
                ? [{ value: "", label: "暂无镜像" }]
                : availableImages.map((image) => ({ value: image.id, label: `${image.name} · ${image.architecture}` }))
            }
          />
        </Field>
        <TextField label="CPU" value={form.cpuCores} onChange={(cpuCores) => onFormChange({ ...form, cpuCores })} required inputMode="numeric" />
        <TextField label="内存 MiB" value={form.memoryMib} onChange={(memoryMib) => onFormChange({ ...form, memoryMib })} required inputMode="numeric" />
        <TextField label="磁盘 GB" value={form.diskGb} onChange={(diskGb) => onFormChange({ ...form, diskGb })} required inputMode="numeric" />
        <Field label="网络模式">
          <Select
            value={form.networkMode}
            onValueChange={(networkMode) =>
              onFormChange({ ...form, networkMode: networkMode as "user_nat" | "bridge_tap" })
            }
            options={[
              { value: "user_nat", label: "User NAT" },
              { value: "bridge_tap", label: "Bridge TAP" },
            ]}
          />
        </Field>
        {form.networkMode === "bridge_tap" ? (
          <TextField label="Bridge" value={form.bridge} onChange={(bridge) => onFormChange({ ...form, bridge })} required placeholder="bridge0" />
        ) : null}
      </div>
      <Field label="端口转发">
        <textarea
          value={form.portForwards}
          onChange={(event) => onFormChange({ ...form, portForwards: event.target.value })}
          placeholder="tcp:2222:22，每行一个"
          className="min-h-20 w-full rounded-md border bg-background px-3 py-2 text-sm outline-none ring-offset-background focus-visible:ring-2 focus-visible:ring-ring"
        />
      </Field>
    </>
  );
}

function SnapshotFields({
  form,
  onFormChange,
}: {
  form: VmFormState;
  onFormChange: (form: VmFormState) => void;
}) {
  return (
    <div className="grid gap-4 sm:grid-cols-2">
      <TextField label="快照名称" value={form.snapshotName} onChange={(snapshotName) => onFormChange({ ...form, snapshotName })} required />
      <TextField label="描述" value={form.snapshotDescription} onChange={(snapshotDescription) => onFormChange({ ...form, snapshotDescription })} />
    </div>
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
  placeholder,
  inputMode,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  required?: boolean;
  placeholder?: string;
  inputMode?: HTMLAttributes<HTMLInputElement>["inputMode"];
}) {
  return (
    <Field label={label}>
      <input
        required={required}
        value={value}
        inputMode={inputMode}
        placeholder={placeholder}
        onChange={(event) => onChange(event.target.value)}
        className="h-9 w-full rounded-md border bg-background px-3 text-sm outline-none ring-offset-background placeholder:text-muted-foreground focus-visible:ring-2 focus-visible:ring-ring"
      />
    </Field>
  );
}

function machineRow(machine: VirtualMachine, hostNames: Map<string, string>): VmRow {
  return {
    id: machine.id,
    hostId: machine.host_id,
    hostLabel: hostNames.get(machine.host_id) ?? machine.host_id,
    name: machine.name,
    secondary: machine.vm_ref,
    status: resourceStatus(machine.status),
    detailA: machine.image,
    detailB: `${machine.cpu_cores} vCPU / ${formatMemory(machine.memory_mib)} / ${machine.disk_gb} GB`,
    detailC: `${machine.provider} · ${machine.networks[0]?.mode ?? "user_nat"}`,
    raw: machine,
  };
}

function imageRow(image: VirtualMachineImage, hostNames: Map<string, string>): VmRow {
  return {
    id: `${image.host_id ?? "store"}:${image.id}`,
    hostId: image.host_id ?? "",
    hostLabel: image.host_id ? hostNames.get(image.host_id) ?? image.host_id : "控制平面",
    name: image.name,
    secondary: image.id,
    status: "running",
    detailA: image.architecture,
    detailB: image.os_family ?? "-",
    detailC: image.path,
    raw: image,
  };
}

function snapshotRow(
  snapshot: VirtualMachineSnapshot,
  machine: VirtualMachine | undefined,
  hostNames: Map<string, string>,
): VmRow {
  return {
    id: snapshot.id,
    hostId: machine?.host_id ?? "",
    hostLabel: machine ? hostNames.get(machine.host_id) ?? machine.host_id : "-",
    name: snapshot.name,
    secondary: snapshot.id,
    status: "stopped",
    detailA: machine?.name ?? snapshot.vm_id,
    detailB: snapshot.description ?? "-",
    detailC: new Date(snapshot.created_at).toLocaleString("zh-CN"),
    raw: snapshot,
  };
}

function templateRow(template: VirtualMachineTemplate): VmRow {
  return {
    id: template.id,
    hostId: "",
    hostLabel: "控制平面",
    name: template.name,
    secondary: template.id,
    status: "running",
    detailA: template.image_id,
    detailB: `${template.cpu_cores} vCPU / ${formatMemory(template.memory_mib)} / ${template.disk_gb} GB`,
    detailC: template.description || "-",
    raw: template,
  };
}

function resourceStatus(status: VirtualMachine["status"]): ResourceStatus {
  if (status === "running") {
    return "running";
  }
  if (status === "starting" || status === "paused" || status === "stopping" || status === "failed") {
    return "warning";
  }
  return "stopped";
}

function formatMemory(memoryMib: number) {
  if (memoryMib >= 1024) {
    return `${Math.round(memoryMib / 1024)} GB`;
  }
  return `${memoryMib} MB`;
}

function hostLabel(host: Host) {
  return host.display_name || host.hostname;
}

function shortId(id: string) {
  return id.slice(0, 8);
}

function parsePortForwards(value: string) {
  return value
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => {
      const [protocol = "tcp", hostPort = "0", guestPort = "0"] = line.split(":");
      return {
        protocol,
        host_port: Number(hostPort),
        guest_port: Number(guestPort),
      };
    })
    .filter((port) => port.host_port > 0 && port.guest_port > 0);
}
