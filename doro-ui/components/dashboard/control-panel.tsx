import { OverviewPage } from "@/components/dashboard/overview/overview-page";
import { AppShell } from "@/components/layout/app-shell";
import type { UserSummary } from "@/types/api";

const previewUser: UserSummary = {
  id: "preview",
  username: "admin",
  display_name: "Doro Admin",
  role: "admin",
};

export function ControlPanel() {
  return (
    <AppShell user={previewUser}>
      <OverviewPage />
    </AppShell>
  );
}
