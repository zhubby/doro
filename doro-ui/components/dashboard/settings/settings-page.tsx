import { PageSection } from "@/components/admin/page-section";
import { SettingList } from "@/components/admin/setting-list";
import { PageContainer } from "@/components/layout/page-container";
import { panelSettings } from "@/lib/mock-data";
import type { SettingsResponse } from "@/types/api";
import { useTranslations } from "next-intl";

type SettingsPageProps = {
  settings?: SettingsResponse | null;
  apiError?: string | null;
};

export function SettingsPage({ settings, apiError }: SettingsPageProps) {
  const t = useTranslations("settings");
  const tCommon = useTranslations("common");
  const controlPlaneSettings = settings
    ? [
        {
          id: "approval-policy",
          label: t("approvalPolicy"),
          value: settings.approval_policy,
          helper: t("approvalPolicyHelper"),
          action: tCommon("actions.view"),
        },
        {
          id: "agent-transport",
          label: t("agentTransport"),
          value: settings.agent_transport,
          helper: t("agentTransportHelper"),
          action: tCommon("actions.view"),
        },
        {
          id: "database",
          label: t("database"),
          value: settings.database,
          helper: t("databaseHelper"),
          action: tCommon("actions.view"),
        },
      ]
    : panelSettings;

  return (
    <PageContainer>
      {apiError ? (
        <div className="rounded-lg border border-destructive/30 p-4 text-sm text-muted-foreground">
          {tCommon("errors.controlPlaneUnavailable", { error: apiError })}
        </div>
      ) : null}
      <PageSection>
        <SettingList settings={controlPlaneSettings} />
      </PageSection>
    </PageContainer>
  );
}
