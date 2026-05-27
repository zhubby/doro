import { PageSection } from "@/components/admin/page-section";
import { SettingList } from "@/components/admin/setting-list";
import { PageContainer } from "@/components/layout/page-container";
import { panelSettings } from "@/lib/mock-data";

export function SettingsPage() {
  return (
    <PageContainer>
      <PageSection
        title="面板设置"
        description="复刻 1Panel 设置页的高价值配置项，当前以本地状态表达交互。"
      >
        <SettingList settings={panelSettings} />
      </PageSection>
    </PageContainer>
  );
}
