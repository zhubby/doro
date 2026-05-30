"use client";

import { useEffect, useState } from "react";

import { SystemPage } from "@/components/dashboard/system/system-page";
import { getHosts, getLatestHostMetric } from "@/lib/control-plane-api";
import type { Host, MetricSnapshot } from "@/types/api";

export default function System() {
  const [hosts, setHosts] = useState<Host[]>([]);
  const [metric, setMetric] = useState<MetricSnapshot | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function load() {
      const hostsResult = await getHosts();
      const hostItems = hostsResult.data?.items ?? [];
      const selectedHost =
        hostItems.find((host) => host.status === "online") ?? hostItems[0];
      const metricResult = selectedHost
        ? await getLatestHostMetric(selectedHost.id)
        : { data: null, error: null };

      if (cancelled) {
        return;
      }
      setHosts(hostItems);
      setMetric(metricResult.data?.item ?? null);
      setError(hostsResult.error ?? metricResult.error);
    }

    load();

    return () => {
      cancelled = true;
    };
  }, []);

  return <SystemPage hosts={hosts} metric={metric} apiError={error} />;
}
