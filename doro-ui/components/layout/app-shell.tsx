"use client";

import { usePathname } from "next/navigation";

import { DashboardHeader } from "@/components/layout/dashboard-header";
import { Sidebar } from "@/components/layout/sidebar";
import { getNavigationItem } from "@/lib/navigation";
import { useTheme } from "@/hooks/use-theme";

export function AppShell({ children }: { children: React.ReactNode }) {
  const pathname = usePathname();
  const { isDark, toggleTheme } = useTheme();
  const activeItem = getNavigationItem(pathname);

  return (
    <div className="min-h-screen bg-background text-foreground">
      <div className="grid min-h-screen lg:grid-cols-[17rem_1fr]">
        <Sidebar pathname={pathname} />
        <main className="flex min-w-0 flex-col">
          <DashboardHeader
            activeItem={activeItem}
            isDark={isDark}
            onToggleTheme={toggleTheme}
          />
          {children}
        </main>
      </div>
    </div>
  );
}
