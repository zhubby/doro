import { PageSection } from "@/components/admin/page-section";
import { SettingList } from "@/components/admin/setting-list";
import { PageContainer } from "@/components/layout/page-container";
import { panelSettings } from "@/lib/mock-data";
import type { SettingsResponse } from "@/types/api";

type SettingsPageProps = {
  settings?: SettingsResponse | null;
  apiError?: string | null;
};

export function SettingsPage({ settings, apiError }: SettingsPageProps) {
  const controlPlaneSettings = settings
    ? [
        {
          id: "approval-policy",
          label: "审批策略",
          value: settings.approval_policy,
          helper: "由控制平面返回的高风险操作审批策略。",
          action: "查看",
        },
        {
          id: "agent-transport",
          label: "Agent 通道",
          value: settings.agent_transport,
          helper: "Agent 与控制平面之间的通信协议。",
          action: "查看",
        },
        {
          id: "database",
          label: "数据库",
          value: settings.database,
          helper: "控制平面当前使用的持久化后端。",
          action: "查看",
        },
      ]
    : panelSettings;

  return (
    <PageContainer>
      {apiError ? (
        <div className="rounded-lg border border-destructive/30 p-4 text-sm text-muted-foreground">
          控制平面暂不可用：{apiError}
        </div>
      ) : null}
      <PageSection
        title="面板设置"
        description="控制平面公开的运行配置。"
      >
        <SettingList settings={controlPlaneSettings} />
      </PageSection>
    </PageContainer>
  );
}
