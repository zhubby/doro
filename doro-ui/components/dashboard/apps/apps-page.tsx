"use client";

import {
  Cpu,
  HardDrive,
  MonitorPlay,
  Network,
  Plus,
  RefreshCw,
  Search,
  Server,
  Settings2,
} from "lucide-react";
import type { ElementType } from "react";
import { useMemo, useState } from "react";

import { FilterChips, type FilterChip } from "@/components/admin/filter-chips";
import { ResourceStatusBadge } from "@/components/admin/data-table";
import { PageSection } from "@/components/admin/page-section";
import { Toolbar } from "@/components/admin/toolbar";
import { PageContainer } from "@/components/layout/page-container";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";
import { cn } from "@/lib/utils";
import { virtualMachines } from "@/lib/mock-data";
import type { ResourceStatus, VirtualMachineResource } from "@/types/dashboard";

type AppsPageProps = {
  apiError?: string | null;
};

const labels: Record<ResourceStatus | "all", string> = {
  all: "全部",
  running: "运行中",
  stopped: "已停止",
  warning: "需关注",
};

function parseUsage(value: string) {
  const match = value.match(/(\d+)%/);

  return match ? Number(match[1]) : 0;
}

function MetricPill({
  icon: Icon,
  label,
  value,
}: {
  icon: ElementType;
  label: string;
  value: string;
}) {
  const progress = parseUsage(value);

  return (
    <div className="rounded-lg border bg-background p-3">
      <div className="mb-2 flex items-center justify-between gap-3">
        <div className="flex items-center gap-2 text-xs text-muted-foreground">
          <Icon className="size-3.5" aria-hidden="true" />
          {label}
        </div>
        <span className="text-xs font-medium">{value}</span>
      </div>
      <Progress value={progress} className="h-1.5" />
    </div>
  );
}

function VirtualMachineCard({ machine }: { machine: VirtualMachineResource }) {
  return (
    <article className="rounded-lg border bg-card p-4 transition-colors hover:bg-muted/30">
      <div className="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
        <div className="min-w-0 space-y-3">
          <div className="flex flex-wrap items-center gap-2">
            <div className="flex size-10 items-center justify-center rounded-md bg-muted">
              <MonitorPlay className="size-5 text-muted-foreground" aria-hidden="true" />
            </div>
            <div className="min-w-0">
              <h3 className="truncate text-sm font-semibold">{machine.name}</h3>
              <p className="truncate text-xs text-muted-foreground">{machine.id}</p>
            </div>
            <ResourceStatusBadge status={machine.status} />
          </div>
          <div className="flex flex-wrap gap-2 text-xs text-muted-foreground">
            <Badge variant="outline">{machine.image}</Badge>
            <Badge variant="outline">{machine.host}</Badge>
            <Badge variant="outline">{machine.address}</Badge>
          </div>
        </div>

        <div className="flex flex-wrap gap-2">
          <Button size="sm" variant="outline">
            控制台
          </Button>
          <Button size="sm" variant="outline">
            快照
          </Button>
          <Button size="sm">管理</Button>
        </div>
      </div>

      <div className="mt-4 grid gap-3 md:grid-cols-3">
        <MetricPill icon={Cpu} label="CPU" value={machine.cpu} />
        <MetricPill icon={Server} label="内存" value={machine.memory} />
        <MetricPill icon={HardDrive} label="磁盘" value={machine.disk} />
      </div>

      <div className="mt-4 flex flex-wrap items-center justify-between gap-3 border-t pt-3 text-xs text-muted-foreground">
        <span>运行时间：{machine.uptime}</span>
        <span>更新时间：{machine.updatedAt}</span>
      </div>
    </article>
  );
}

export function AppsPage({ apiError }: AppsPageProps) {
  const [activeStatus, setActiveStatus] = useState<ResourceStatus | "all">("all");
  const filters = useMemo<FilterChip[]>(() => {
    const statuses: Array<ResourceStatus | "all"> = [
      "all",
      "running",
      "warning",
      "stopped",
    ];

    return statuses.map((status) => ({
      value: status,
      label: labels[status],
      count:
        status === "all"
          ? virtualMachines.length
          : virtualMachines.filter((machine) => machine.status === status).length,
    }));
  }, []);
  const filteredMachines = useMemo(() => {
    if (activeStatus === "all") {
      return virtualMachines;
    }

    return virtualMachines.filter((machine) => machine.status === activeStatus);
  }, [activeStatus]);
  const runningCount = virtualMachines.filter(
    (machine) => machine.status === "running",
  ).length;
  const warningCount = virtualMachines.filter(
    (machine) => machine.status === "warning",
  ).length;

  return (
    <PageContainer>
      {apiError ? (
        <div className="rounded-lg border border-destructive/30 p-4 text-sm text-muted-foreground">
          控制平面暂不可用：{apiError}
        </div>
      ) : null}

      <div className="grid gap-4 md:grid-cols-3">
        {[
          { label: "虚拟机", value: virtualMachines.length, helper: "跨 2 台宿主机" },
          { label: "运行中", value: runningCount, helper: "可直接进入控制台" },
          { label: "需关注", value: warningCount, helper: "资源或快照策略异常" },
        ].map((stat) => (
          <div key={stat.label} className="rounded-md bg-muted/35 px-4 py-3">
            <p className="text-sm text-muted-foreground">{stat.label}</p>
            <p className="mt-1 text-lg font-semibold leading-6">{stat.value}</p>
            <p className="mt-1 text-sm text-muted-foreground">{stat.helper}</p>
          </div>
        ))}
      </div>

      <PageSection contentClassName="space-y-4">
        <FilterChips
          filters={filters}
          value={activeStatus}
          onValueChange={(value) => setActiveStatus(value as ResourceStatus | "all")}
        />
      </PageSection>

      <PageSection contentClassName="space-y-4">
        <Toolbar
          left={
            <>
              <Button>
                <Plus className="size-4" aria-hidden="true" />
                创建虚拟机
              </Button>
              <Button variant="outline">批量启动</Button>
              <Button variant="outline">批量停止</Button>
              <Button variant="outline">创建快照</Button>
            </>
          }
          right={
            <>
              <Button variant="outline">
                <Search className="size-4" aria-hidden="true" />
                搜索
              </Button>
              <Button variant="outline" size="icon" aria-label="刷新">
                <RefreshCw className="size-4" aria-hidden="true" />
              </Button>
              <Button variant="outline" size="icon" aria-label="视图设置">
                <Settings2 className="size-4" aria-hidden="true" />
              </Button>
            </>
          }
        />

        <div className="grid gap-4">
          {filteredMachines.map((machine) => (
            <VirtualMachineCard key={machine.id} machine={machine} />
          ))}
        </div>
      </PageSection>

      <PageSection
        title="网络与镜像"
        description="虚拟机默认网络、镜像模板和宿主机池的占用情况。"
      >
        <div className="grid gap-3 md:grid-cols-3">
          {[
            {
              icon: Network,
              label: "默认网络",
              value: "bridge-home",
              helper: "10.0.1.0/24 · DHCP 已启用",
            },
            {
              icon: HardDrive,
              label: "镜像缓存",
              value: "6 个模板",
              helper: "Ubuntu / Debian / Fedora / HAOS",
            },
            {
              icon: Server,
              label: "宿主机池",
              value: "2 台可调度",
              helper: "剩余 14 vCPU / 38 GB 内存",
            },
          ].map((item) => {
            const Icon = item.icon;

            return (
              <div
                key={item.label}
                className={cn("rounded-lg border p-4", "bg-background")}
              >
                <div className="flex items-center gap-2 text-sm font-medium">
                  <Icon className="size-4 text-muted-foreground" aria-hidden="true" />
                  {item.label}
                </div>
                <p className="mt-3 text-lg font-semibold">{item.value}</p>
                <p className="mt-1 text-xs text-muted-foreground">{item.helper}</p>
              </div>
            );
          })}
        </div>
      </PageSection>
    </PageContainer>
  );
}
