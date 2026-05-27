import { OverviewPage } from "@/components/dashboard/overview/overview-page";
import { AppShell } from "@/components/layout/app-shell";

export function ControlPanel() {
  return (
    <AppShell>
      <OverviewPage />
    </AppShell>
  );
}
