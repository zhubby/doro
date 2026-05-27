import { OverviewPage } from "@/components/dashboard/overview/overview-page";
import { getApps, getApprovals, getHosts, getTasks } from "@/lib/control-plane-api";

export const dynamic = "force-dynamic";

export default async function Home() {
  const [hosts, tasks, approvals, apps] = await Promise.all([
    getHosts(),
    getTasks(),
    getApprovals(),
    getApps(),
  ]);
  const apiError = hosts.error ?? tasks.error ?? approvals.error ?? apps.error;

  return (
    <OverviewPage
      hosts={hosts.data?.items ?? []}
      tasks={tasks.data?.items ?? []}
      approvals={approvals.data?.items ?? []}
      apps={apps.data?.items ?? []}
      apiError={apiError}
    />
  );
}
