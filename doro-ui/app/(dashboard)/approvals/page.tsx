import { ApprovalsPage } from "@/components/dashboard/approvals/approvals-page";
import { getApprovals } from "@/lib/control-plane-api";

export const dynamic = "force-dynamic";

export default async function ApprovalsRoute() {
  const approvals = await getApprovals();

  return (
    <ApprovalsPage
      approvals={approvals.data?.items ?? []}
      apiError={approvals.error}
    />
  );
}
