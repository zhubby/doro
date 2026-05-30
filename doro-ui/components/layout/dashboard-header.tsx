"use client";

import { useState } from "react";
import { Moon, Sun } from "lucide-react";
import { useTranslations } from "next-intl";

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
  const tCommon = useTranslations("common");
  const tNav = useTranslations("navigation");
  const title = tNav(`items.${activeItem.id}.label`);
  const description = tNav(`items.${activeItem.id}.description`);
  const themeLabel = isDark ? tCommon("theme.toLight") : tCommon("theme.toDark");

  return (
    <header className="flex h-20 shrink-0 flex-col justify-center gap-2 border-b px-6 md:flex-row md:items-center md:justify-between">
      <div>
        <div className="flex items-center gap-2">
          <h1 className="text-2xl font-semibold tracking-tight">
            {title}
          </h1>
          {activeItem.count ? (
            <Badge variant="secondary">{activeItem.count}</Badge>
          ) : null}
        </div>
        <p className="text-sm text-muted-foreground">
          {description}
        </p>
      </div>
      <div className="flex flex-wrap gap-2">
        <SearchCommand open={searchOpen} onOpenChange={setSearchOpen} />
        <Button
          variant="outline"
          size="icon"
          onClick={onToggleTheme}
          aria-label={themeLabel}
          title={themeLabel}
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
