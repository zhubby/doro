"use client";

import { Boxes, ExternalLink } from "lucide-react";

import { ResourceStatusBadge } from "@/components/admin/data-table";
import { PageSection } from "@/components/admin/page-section";
import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";
import type { ContainerResource } from "@/types/dashboard";

function parseUsage(value: string) {
  const match = value.match(/(\d+(?:\.\d+)?)%/);

  return match ? Number(match[1]) : 0;
}

type ContainerListProps = {
  containers: ContainerResource[];
  className?: string;
};

export function ContainerList({ containers, className }: ContainerListProps) {
  return (
    <PageSection
      title="容器"
      description="运行中的容器与资源占用"
      className={className}
      toolbar={
        <Button size="sm" variant="outline">
          <ExternalLink className="size-4" aria-hidden="true" />
          全部
        </Button>
      }
    >
      <div className="space-y-3">
        {containers.map((container) => (
          <div key={container.id} className="rounded-lg border p-3">
            <div className="flex items-start gap-3">
              <div className="flex size-10 shrink-0 items-center justify-center rounded-md bg-muted">
                <Boxes className="size-5 text-muted-foreground" aria-hidden="true" />
              </div>
              <div className="min-w-0 flex-1">
                <div className="flex flex-wrap items-center gap-2">
                  <p className="truncate text-sm font-medium">{container.name}</p>
                  <ResourceStatusBadge status={container.status} />
                </div>
                <p className="mt-1 truncate text-xs text-muted-foreground">
                  {container.image}
                </p>
                <p className="mt-1 truncate text-xs text-muted-foreground">
                  {container.ports}
                </p>
              </div>
            </div>

            <div className="mt-3 grid gap-3 text-xs sm:grid-cols-2">
              <div>
                <div className="mb-1 flex justify-between text-muted-foreground">
                  <span>CPU</span>
                  <span>{container.cpu}</span>
                </div>
                <Progress value={parseUsage(container.cpu)} className="h-1.5" />
              </div>
              <div>
                <div className="mb-1 flex justify-between text-muted-foreground">
                  <span>内存</span>
                  <span>{container.memory}</span>
                </div>
                <Progress
                  value={container.memory === "0 MB" ? 0 : 32}
                  className="h-1.5"
                />
              </div>
            </div>

            <div className="mt-3 flex items-center justify-between border-t pt-2 text-xs text-muted-foreground">
              <span>{container.source}</span>
              <span>{container.updatedAt}</span>
            </div>
          </div>
        ))}
      </div>
    </PageSection>
  );
}
