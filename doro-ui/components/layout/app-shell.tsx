"use client";

import { DashboardHeader } from "@/components/layout/dashboard-header";
import { PageTransition } from "@/components/layout/page-transition";
import { Sidebar } from "@/components/layout/sidebar";
import { usePathname } from "@/i18n/navigation";
import { getNavigationItem } from "@/lib/navigation";
import { useTheme } from "@/hooks/use-theme";
import type { UserSummary } from "@/types/api";

export function AppShell({
  children,
  user,
}: {
  children: React.ReactNode;
  user: UserSummary;
}) {
  const pathname = usePathname();
  const { isDark, toggleTheme } = useTheme();
  const activeItem = getNavigationItem(pathname);

  return (
    <div className="h-dvh overflow-hidden bg-background text-foreground">
      <div className="grid h-full min-h-0 lg:grid-cols-[17rem_1fr]">
        <Sidebar pathname={pathname} user={user} />
        <main className="flex min-h-0 min-w-0 flex-col overflow-hidden">
          <DashboardHeader
            activeItem={activeItem}
            isDark={isDark}
            onToggleTheme={toggleTheme}
          />
          <PageTransition pathname={pathname}>{children}</PageTransition>
        </main>
      </div>
    </div>
  );
}
