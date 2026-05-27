import { Lock, Sparkles, Wrench } from "lucide-react";

import { PageSection } from "@/components/admin/page-section";
import { PageContainer } from "@/components/layout/page-container";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { tools } from "@/lib/mock-data";
import type { ToolEntry } from "@/types/dashboard";

function ToolStatusBadge({ status }: { status: ToolEntry["status"] }) {
  if (status === "ready") {
    return <Badge>可用</Badge>;
  }

  if (status === "beta") {
    return <Badge variant="secondary">Beta</Badge>;
  }

  return <Badge variant="outline">待接入</Badge>;
}

export function ToolsPage() {
  return (
    <PageContainer>
      <PageSection
        title="工具箱"
        description="用卡片网格承接 1Panel 工具箱的系统工具、诊断工具和快捷入口。"
        toolbar={
          <Button variant="outline">
            <Sparkles className="size-4" aria-hidden="true" />
            推荐工具
          </Button>
        }
      >
        <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
          {tools.map((tool) => {
            const Icon = tool.status === "locked" ? Lock : Wrench;

            return (
              <div key={tool.id} className="rounded-lg border p-4">
                <div className="flex items-start justify-between gap-3">
                  <div className="flex items-center gap-3">
                    <div className="flex size-10 items-center justify-center rounded-md bg-muted">
                      <Icon
                        className="size-5 text-muted-foreground"
                        aria-hidden="true"
                      />
                    </div>
                    <div>
                      <p className="text-sm font-medium">{tool.title}</p>
                      <p className="text-xs text-muted-foreground">
                        {tool.category}
                      </p>
                    </div>
                  </div>
                  <ToolStatusBadge status={tool.status} />
                </div>
                <p className="mt-4 text-sm text-muted-foreground">
                  {tool.description}
                </p>
                <Button
                  className="mt-4 w-full"
                  variant={tool.status === "ready" ? "default" : "outline"}
                >
                  {tool.status === "locked" ? "查看说明" : "打开"}
                </Button>
              </div>
            );
          })}
        </div>
      </PageSection>
    </PageContainer>
  );
}
