"use client";

import { useState } from "react";
import { Moon, Sun } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { SearchCommand } from "@/components/layout/search-command";
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
  const [searchOpen, setSearchOpen] = useState(false);

  return (
    <header className="flex h-20 shrink-0 flex-col justify-center gap-2 border-b px-6 md:flex-row md:items-center md:justify-between">
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
        <SearchCommand open={searchOpen} onOpenChange={setSearchOpen} />
        <Button
          variant="outline"
          size="icon"
          onClick={onToggleTheme}
          aria-label={isDark ? "切换到浅色主题" : "切换到深色主题"}
          title={isDark ? "切换到浅色主题" : "切换到深色主题"}
        >
          {isDark ? (
            <Sun className="size-4" aria-hidden="true" />
          ) : (
            <Moon className="size-4" aria-hidden="true" />
          )}
        </Button>
      </div>
    </header>
  );
}
