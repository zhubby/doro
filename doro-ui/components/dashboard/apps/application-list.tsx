"use client";

import { useMemo, useState } from "react";

import { PageSection } from "@/components/admin/page-section";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type { Application, AppState } from "@/types/dashboard";
import { useTranslations } from "next-intl";

function getApplicationAction(state: AppState, t: ReturnType<typeof useTranslations>) {
  if (state === "running") {
    return t("actions.manage");
  }

  if (state === "installed" || state === "upgrade") {
    return state === "upgrade" ? t("actions.upgrade") : t("actions.start");
  }

  return t("actions.install");
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
  const t = useTranslations("common.status");

  if (state === "running") {
    return <Badge>{t("running")}</Badge>;
  }

  if (state === "installed") {
    return <Badge variant="secondary">{t("installed")}</Badge>;
  }

  if (state === "upgrade") {
    return <Badge variant="secondary">{t("upgrade")}</Badge>;
  }

  return <Badge variant="outline">{t("available")}</Badge>;
}

type ApplicationListProps = {
  title: string;
  description: string;
  applications: Application[];
  filter?: "all" | "installed" | "upgrade";
  compact?: boolean;
  className?: string;
  contentClassName?: string;
};

export function ApplicationList({
  title,
  description,
  applications,
  filter = "all",
  compact = false,
  className,
  contentClassName,
}: ApplicationListProps) {
  const tCommon = useTranslations("common");
  const tResources = useTranslations("resources.applications");
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
      className={className}
      contentClassName={contentClassName}
      toolbar={
        compact ? (
          <Button size="sm" variant="outline">
            {tResources("all")}
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
                {getApplicationAction(state, tCommon)}
              </Button>
            </div>
          );
        })}
      </div>
    </PageSection>
  );
}
