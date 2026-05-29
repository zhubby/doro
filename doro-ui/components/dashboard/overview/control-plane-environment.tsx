"use client";

import { useEffect, useState } from "react";
import { Server, ShieldCheck } from "lucide-react";

import { PageSection } from "@/components/admin/page-section";
import { Badge } from "@/components/ui/badge";
import type { ControlPlaneEnvironment } from "@/types/api";

type ControlPlaneEnvironmentProps = {
  environment: ControlPlaneEnvironment | null;
  className?: string;
};

function formatDateTime(value: string | null) {
  if (!value) {
    return "-";
  }

  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return new Intl.DateTimeFormat("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  }).format(date);
}

function formatDuration(totalSeconds: number) {
  if (!Number.isFinite(totalSeconds) || totalSeconds <= 0) {
    return "-";
  }

  const days = Math.floor(totalSeconds / 86_400);
  const hours = Math.floor((totalSeconds % 86_400) / 3_600);
  const minutes = Math.floor((totalSeconds % 3_600) / 60);
  const seconds = Math.floor(totalSeconds % 60);

  return `${days}天 ${hours}小时 ${minutes}分钟 ${seconds}秒`;
}

function runtimeSeconds(environment: ControlPlaneEnvironment | null, now: Date) {
  if (!environment) {
    return 0;
  }

  if (environment.booted_at) {
    const bootedAt = new Date(environment.booted_at).getTime();
    if (!Number.isNaN(bootedAt)) {
      return Math.max(0, Math.floor((now.getTime() - bootedAt) / 1000));
    }
  }

  return environment.uptime_seconds;
}

export function ControlPlaneEnvironmentPanel({
  environment,
  className,
}: ControlPlaneEnvironmentProps) {
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    const timer = window.setInterval(() => setNow(new Date()), 1000);
    return () => window.clearInterval(timer);
  }, []);

  const rows = [
    ["主机名称", environment?.hostname],
    ["发行版本", environment?.os_version],
    ["内核版本", environment?.kernel_version],
    ["系统类型", environment?.architecture],
    ["主机地址", environment?.host_address],
    ["启动时间", formatDateTime(environment?.booted_at ?? null)],
    ["运行时间", formatDuration(runtimeSeconds(environment, now))],
  ];

  return (
    <PageSection
      title="控制平面环境"
      description="control-plane 所在主机状态"
      className={className}
      contentClassName="flex flex-1 flex-col"
      toolbar={
        <Badge variant="outline" className="gap-1.5">
          <ShieldCheck className="size-3.5" aria-hidden="true" />
          本机
        </Badge>
      }
    >
      <div className="flex flex-1 flex-col justify-between gap-4">
        <div className="flex items-center gap-3 rounded-lg border bg-muted/30 p-3">
          <div className="flex size-10 shrink-0 items-center justify-center rounded-md bg-background">
            <Server className="size-5 text-muted-foreground" aria-hidden="true" />
          </div>
          <div className="min-w-0">
            <p className="truncate text-sm font-medium">
              {environment?.hostname ?? "等待控制平面信息"}
            </p>
            <p className="mt-1 truncate text-xs text-muted-foreground">
              {environment
                ? `${environment.os_version} / ${environment.architecture}`
                : "环境信息暂不可用"}
            </p>
          </div>
        </div>

        <dl className="divide-y rounded-lg border">
          {rows.map(([label, value]) => (
            <div
              key={label}
              className="grid grid-cols-[5.5rem_minmax(0,1fr)] gap-3 px-3 py-2.5 text-sm"
            >
              <dt className="text-muted-foreground">{label}</dt>
              <dd className="min-w-0 break-words font-medium">{value || "-"}</dd>
            </div>
          ))}
        </dl>
      </div>
    </PageSection>
  );
}
