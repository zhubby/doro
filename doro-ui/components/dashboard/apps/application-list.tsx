"use client";

import { useMemo, useState } from "react";

import { PageSection } from "@/components/admin/page-section";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type { Application, AppState } from "@/types/dashboard";

function getApplicationAction(state: AppState) {
  if (state === "running") {
    return "管理";
  }

  if (state === "installed" || state === "upgrade") {
    return state === "upgrade" ? "升级" : "启动";
  }

  return "安装";
}

function getNextApplicationState(state: AppState): AppState {
  if (state === "available") {
    return "installed";
  }

  if (state === "upgrade") {
    return "installed";
  }

  return "running";
}

export function StatusBadge({ state }: { state: AppState }) {
  if (state === "running") {
    return <Badge>运行中</Badge>;
  }

  if (state === "installed") {
    return <Badge variant="secondary">已安装</Badge>;
  }

  if (state === "upgrade") {
    return <Badge variant="secondary">可升级</Badge>;
  }

  return <Badge variant="outline">可安装</Badge>;
}

type ApplicationListProps = {
  title: string;
  description: string;
  applications: Application[];
  filter?: "all" | "installed" | "upgrade";
  compact?: boolean;
};

export function ApplicationList({
  title,
  description,
  applications,
  filter = "all",
  compact = false,
}: ApplicationListProps) {
  const [applicationStates, setApplicationStates] = useState(
    () => new Map(applications.map((application) => [application.id, application.state])),
  );
  const filteredApplications = useMemo(() => {
    if (filter === "installed") {
      return applications.filter((application) =>
        ["installed", "running", "upgrade"].includes(
          applicationStates.get(application.id) ?? application.state,
        ),
      );
    }

    if (filter === "upgrade") {
      return applications.filter((application) => application.updateAvailable);
    }

    return applications;
  }, [applicationStates, applications, filter]);

  return (
    <PageSection
      title={title}
      description={description}
      toolbar={
        compact ? (
          <Button size="sm" variant="outline">
            全部
          </Button>
        ) : null
      }
    >
      <div className={compact ? "space-y-4" : "grid gap-4 md:grid-cols-2"}>
        {filteredApplications.map((application) => {
          const Icon = application.icon;
          const state = applicationStates.get(application.id) ?? application.state;

          return (
            <div
              key={application.id}
              className="flex items-center gap-3 rounded-lg border p-3"
            >
              <div className="flex size-10 items-center justify-center rounded-md bg-muted">
                <Icon
                  className="size-5 text-muted-foreground"
                  aria-hidden="true"
                />
              </div>
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                  <p className="truncate text-sm font-medium">
                    {application.name}
                  </p>
                  <StatusBadge state={state} />
                </div>
                <p className="truncate text-xs text-muted-foreground">
                  {application.version}
                </p>
                <p className="truncate text-xs text-muted-foreground">
                  {application.description}
                </p>
              </div>
              <Button
                size="sm"
                variant={state === "running" ? "outline" : "default"}
                onClick={() =>
                  setApplicationStates((current) =>
                    new Map(current).set(
                      application.id,
                      getNextApplicationState(state),
                    ),
                  )
                }
              >
                {getApplicationAction(state)}
              </Button>
            </div>
          );
        })}
      </div>
    </PageSection>
  );
}
