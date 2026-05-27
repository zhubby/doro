import { AppsPage } from "@/components/dashboard/apps/apps-page";
import { getApps } from "@/lib/control-plane-api";
import { toApplications } from "@/lib/control-plane-mappers";

export const dynamic = "force-dynamic";

export default async function Apps() {
  const apps = await getApps();

  return (
    <AppsPage
      initialApplications={
        apps.data ? toApplications(apps.data.items) : undefined
      }
      apiError={apps.error}
    />
  );
}
