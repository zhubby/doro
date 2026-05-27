"use client";

import { useEffect, useState } from "react";

import { ContainersPage } from "@/components/dashboard/resources/containers-page";
import { getHostContainers, getHosts } from "@/lib/control-plane-api";
import type { HostContainer } from "@/types/api";

export default function Containers() {
  const [containers, setContainers] = useState<HostContainer[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function load() {
      const hostsResult = await getHosts();
      const hostItems = hostsResult.data?.items ?? [];
      const containerResults = await Promise.all(
        hostItems.map((host) => getHostContainers(host.id)),
      );
      if (cancelled) {
        return;
      }
      setContainers(containerResults.flatMap((result) => result.data?.items ?? []));
      setError(
        hostsResult.error ??
          containerResults.find((result) => result.error)?.error ??
          null,
      );
    }

    load();

    return () => {
      cancelled = true;
    };
  }, []);

  return <ContainersPage containers={containers} apiError={error} />;
}
