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
import { useTranslations } from "next-intl";

type AppsPageProps = {
  apiError?: string | null;
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
  const t = useTranslations("resources.apps");
  const tCommon = useTranslations("common.actions");

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
            {t("console")}
          </Button>
          <Button size="sm" variant="outline">
            {tCommon("snapshot")}
          </Button>
          <Button size="sm">{tCommon("manage")}</Button>
        </div>
      </div>

      <div className="mt-4 grid gap-3 md:grid-cols-3">
        <MetricPill icon={Cpu} label="CPU" value={machine.cpu} />
        <MetricPill icon={Server} label={t("stats.memory")} value={machine.memory} />
        <MetricPill icon={HardDrive} label={t("stats.disk")} value={machine.disk} />
      </div>

      <div className="mt-4 flex flex-wrap items-center justify-between gap-3 border-t pt-3 text-xs text-muted-foreground">
        <span>{t("uptime", { value: machine.uptime })}</span>
        <span>{t("updatedAt", { value: machine.updatedAt })}</span>
      </div>
    </article>
  );
}

export function AppsPage({ apiError }: AppsPageProps) {
  const [activeStatus, setActiveStatus] = useState<ResourceStatus | "all">("all");
  const t = useTranslations("resources.apps");
  const tCommon = useTranslations("common");
  const tStatus = useTranslations("common.status");
  const filters = useMemo<FilterChip[]>(() => {
    const statuses: Array<ResourceStatus | "all"> = [
      "all",
      "running",
      "warning",
      "stopped",
    ];

    return statuses.map((status) => ({
      value: status,
      label: status === "all" ? tStatus("all") : tStatus(status),
      count:
        status === "all"
          ? virtualMachines.length
          : virtualMachines.filter((machine) => machine.status === status).length,
    }));
  }, [tStatus]);
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
          {tCommon("errors.controlPlaneUnavailable", { error: apiError })}
        </div>
      ) : null}

      <div className="grid gap-4 md:grid-cols-3">
        {[
          {
            label: t("stats.machines"),
            value: virtualMachines.length,
            helper: t("stats.machinesHelper"),
          },
          {
            label: t("stats.running"),
            value: runningCount,
            helper: t("stats.runningHelper"),
          },
          {
            label: t("stats.warning"),
            value: warningCount,
            helper: t("stats.warningHelper"),
          },
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
                {t("create")}
              </Button>
              <Button variant="outline">{t("batchStart")}</Button>
              <Button variant="outline">{t("batchStop")}</Button>
              <Button variant="outline">{t("snapshot")}</Button>
            </>
          }
          right={
            <>
              <Button variant="outline">
                <Search className="size-4" aria-hidden="true" />
                {tCommon("actions.search")}
              </Button>
              <Button
                variant="outline"
                size="icon"
                aria-label={tCommon("actions.refresh")}
              >
                <RefreshCw className="size-4" aria-hidden="true" />
              </Button>
              <Button variant="outline" size="icon" aria-label={t("viewSettings")}>
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
        title={t("networkTitle")}
        description={t("networkDescription")}
      >
        <div className="grid gap-3 md:grid-cols-3">
          {[
            {
              icon: Network,
              label: t("defaultNetwork"),
              value: "bridge-home",
              helper: t("dhcpEnabled"),
            },
            {
              icon: HardDrive,
              label: t("imageCache"),
              value: t("imageTemplates"),
              helper: "Ubuntu / Debian / Fedora / HAOS",
            },
            {
              icon: Server,
              label: t("hostPool"),
              value: t("hostPoolValue"),
              helper: t("hostPoolHelper"),
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
