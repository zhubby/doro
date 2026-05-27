"use client";

import { Gauge, Moon, Search, Sun } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type { NavigationItem } from "@/types/dashboard";

type DashboardHeaderProps = {
  activeItem: NavigationItem;
  isDark: boolean;
  onToggleTheme: () => void;
};

export function DashboardHeader({
  activeItem,
  isDark,
  onToggleTheme,
}: DashboardHeaderProps) {
  return (
    <header className="flex min-h-16 flex-col gap-3 border-b px-6 py-4 md:flex-row md:items-center md:justify-between">
      <div>
        <div className="flex items-center gap-2">
          <h1 className="text-2xl font-semibold tracking-tight">
            {activeItem.label}
          </h1>
          {activeItem.count ? (
            <Badge variant="secondary">{activeItem.count}</Badge>
          ) : null}
        </div>
        <p className="text-sm text-muted-foreground">
          {activeItem.description}
        </p>
      </div>
      <div className="flex flex-wrap gap-2">
        <Button variant="outline">
          <Search className="size-4" aria-hidden="true" />
          搜索
        </Button>
        <Button variant="outline" onClick={onToggleTheme}>
          {isDark ? (
            <Sun className="size-4" aria-hidden="true" />
          ) : (
            <Moon className="size-4" aria-hidden="true" />
          )}
          {isDark ? "浅色" : "深色"}
        </Button>
        <Button>
          <Gauge className="size-4" aria-hidden="true" />
          快速巡检
        </Button>
      </div>
    </header>
  );
}
