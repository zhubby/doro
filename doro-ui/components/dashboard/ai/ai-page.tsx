import { Bot, Play, Settings2 } from "lucide-react";

import { DataTable, ResourceStatusBadge } from "@/components/admin/data-table";
import { PageSection } from "@/components/admin/page-section";
import { Toolbar } from "@/components/admin/toolbar";
import { PageContainer } from "@/components/layout/page-container";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { aiAgents } from "@/lib/mock-data";
import type { AiAgent, ResourceColumn } from "@/types/dashboard";
import { useTranslations } from "next-intl";

export function AiPage() {
  const t = useTranslations("resources.ai");
  const tResources = useTranslations("resources");
  const columns: ResourceColumn<AiAgent>[] = [
    {
      key: "name",
      label: t("agent"),
      render: (row) => (
        <div>
          <p className="font-medium">{row.name}</p>
          <p className="text-xs text-muted-foreground">{row.role}</p>
        </div>
      ),
    },
    {
      key: "status",
      label: tResources("columns.status"),
      render: (row) => <ResourceStatusBadge status={row.status} />,
    },
    { key: "model", label: tResources("columns.model") },
    { key: "lastRun", label: tResources("columns.lastRun") },
  ];

  return (
    <PageContainer
      aside={
        <PageSection title={t("runtimeTitle")} description={t("runtimeDescription")}>
          <div className="space-y-3">
            {[
              t("toolCallsEnabled"),
              t("queueIdle"),
              t("modelRoutingDefault"),
            ].map(
              (item) => (
                <div key={item} className="flex items-center justify-between rounded-lg border p-3">
                  <span className="text-sm">{item}</span>
                  <Badge variant="secondary">{t("normal")}</Badge>
                </div>
              ),
            )}
          </div>
        </PageSection>
      }
    >
      <PageSection contentClassName="space-y-4">
        <Toolbar
          left={
            <>
              <Button>
                <Bot className="size-4" aria-hidden="true" />
                {t("create")}
              </Button>
              <Button variant="outline">
                <Play className="size-4" aria-hidden="true" />
                {t("runTask")}
              </Button>
            </>
          }
          right={
            <Button variant="outline">
              <Settings2 className="size-4" aria-hidden="true" />
              {t("modelSettings")}
            </Button>
          }
        />
        <DataTable
          columns={columns}
          rows={aiAgents}
          actions={[t("runTask"), t("modelSettings")]}
        />
      </PageSection>
    </PageContainer>
  );
}
