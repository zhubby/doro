import { AuthGate } from "@/components/dashboard/auth-gate";
import { AppShell } from "@/components/layout/app-shell";

export default function DashboardLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <AuthGate>
      <AppShell>{children}</AppShell>
    </AuthGate>
  );
}
