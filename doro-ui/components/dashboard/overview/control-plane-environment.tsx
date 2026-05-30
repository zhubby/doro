"use client";

import { useEffect, useState } from "react";
import { Server, ShieldCheck } from "lucide-react";
import { useLocale, useTranslations } from "next-intl";

import { PageSection } from "@/components/admin/page-section";
import { Badge } from "@/components/ui/badge";
import type { ControlPlaneEnvironment } from "@/types/api";

type ControlPlaneEnvironmentProps = {
  environment: ControlPlaneEnvironment | null;
  className?: string;
};

function formatDateTime(value: string | null, locale: string) {
  if (!value) {
    return "-";
  }

  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return new Intl.DateTimeFormat(locale, {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  }).format(date);
}

function formatDuration(totalSeconds: number, locale: string) {
  if (!Number.isFinite(totalSeconds) || totalSeconds <= 0) {
    return "-";
  }

  const days = Math.floor(totalSeconds / 86_400);
  const hours = Math.floor((totalSeconds % 86_400) / 3_600);
  const minutes = Math.floor((totalSeconds % 3_600) / 60);
  const seconds = Math.floor(totalSeconds % 60);

  if (locale === "zh-CN") {
    return `${days}天 ${hours}小时 ${minutes}分钟 ${seconds}秒`;
  }

  return `${days}d ${hours}h ${minutes}m ${seconds}s`;
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
  const locale = useLocale();
  const t = useTranslations("dashboard.environment");
  useEffect(() => {
    const timer = window.setInterval(() => setNow(new Date()), 1000);
    return () => window.clearInterval(timer);
  }, []);

  const rows = [
    [t("hostname"), environment?.hostname],
    [t("osVersion"), environment?.os_version],
    [t("kernelVersion"), environment?.kernel_version],
    [t("architecture"), environment?.architecture],
    [t("hostAddress"), environment?.host_address],
    [t("bootedAt"), formatDateTime(environment?.booted_at ?? null, locale)],
    [t("uptime"), formatDuration(runtimeSeconds(environment, now), locale)],
  ];

  return (
    <PageSection
      title={t("title")}
      description={t("description")}
      className={className}
      contentClassName="flex flex-1 flex-col"
      toolbar={
        <Badge variant="outline" className="gap-1.5">
          <ShieldCheck className="size-3.5" aria-hidden="true" />
          {t("local")}
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
              {environment?.hostname ?? t("waiting")}
            </p>
            <p className="mt-1 truncate text-xs text-muted-foreground">
              {environment
                ? `${environment.os_version} / ${environment.architecture}`
                : t("unavailable")}
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
