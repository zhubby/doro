import { HostsPage } from "@/components/dashboard/hosts/hosts-page";
import { getHosts } from "@/lib/control-plane-api";

export const dynamic = "force-dynamic";

export default async function HostsRoute() {
  const hosts = await getHosts();

  return <HostsPage hosts={hosts.data?.items ?? []} apiError={hosts.error} />;
}
