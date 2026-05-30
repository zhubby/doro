"use client";

import {
  Boxes,
  ChevronUp,
  Languages,
  LogOut,
  Settings,
  UserRound,
} from "lucide-react";
import { useLocale, useTranslations } from "next-intl";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import { Link, usePathname, useRouter } from "@/i18n/navigation";
import { locales, type AppLocale } from "@/i18n/routing";
import { logout } from "@/lib/control-plane-api";
import { navigation } from "@/lib/navigation";
import { cn } from "@/lib/utils";
import type { UserSummary } from "@/types/api";

export function Sidebar({
  pathname,
  user,
}: {
  pathname: string;
  user: UserSummary;
}) {
  const router = useRouter();
  const currentPathname = usePathname();
  const locale = useLocale() as AppLocale;
  const tCommon = useTranslations("common");
  const tNav = useTranslations("navigation");
  const displayName = user.display_name || user.username;
  const initials = displayName.trim().slice(0, 1).toUpperCase();

  async function handleLogout() {
    await logout();
    router.replace("/login");
  }

  return (
    <aside className="min-h-0 border-b bg-card lg:border-b-0 lg:border-r">
      <div className="flex h-full min-h-0 flex-col">
        <div className="flex h-20 shrink-0 items-center gap-3 border-b px-6">
          <div className="flex size-9 items-center justify-center rounded-lg bg-primary text-primary-foreground">
            <Boxes className="size-4" aria-hidden="true" />
          </div>
          <div>
            <p className="text-sm font-semibold">{tCommon("brand.panel")}</p>
            <p className="text-xs text-muted-foreground">
              {tCommon("brand.localConsole")}
            </p>
          </div>
        </div>
        <ScrollArea className="min-h-0 flex-1 px-3 py-4">
          <nav className="grid gap-1" aria-label={tNav("ariaLabel")}>
            {navigation.map((item) => {
              const Icon = item.icon;
              const isActive =
                item.href === "/"
                  ? pathname === "/"
                  : pathname === item.href ||
                    pathname.startsWith(`${item.href}/`);

              return (
                <Button
                  key={item.href}
                  asChild
                  variant={isActive ? "secondary" : "ghost"}
                  className={cn("justify-start", isActive && "font-semibold")}
                >
                  <Link href={item.href}>
                    <Icon className="size-4" aria-hidden="true" />
                    <span>{tNav(`items.${item.id}.label`)}</span>
                    {item.count ? (
                      <Badge variant="outline" className="ml-auto">
                        {item.count}
                      </Badge>
                    ) : null}
                  </Link>
                </Button>
              );
            })}
          </nav>
        </ScrollArea>
        <Separator />
        <div className="p-4">
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button
                variant="ghost"
                className="h-auto w-full justify-start gap-3 rounded-lg border bg-background p-3 text-left shadow-none hover:bg-accent"
                aria-label={tNav("userMenu.open")}
              >
                <span className="flex size-10 shrink-0 items-center justify-center rounded-full bg-primary text-sm font-semibold text-primary-foreground">
                  {initials}
                </span>
                <span className="min-w-0 flex-1">
                  <span className="block truncate text-sm font-semibold">
                    {displayName}
                  </span>
                  <span className="block truncate text-xs text-muted-foreground">
                    @{user.username} · {user.role}
                  </span>
                </span>
                <ChevronUp
                  className="size-4 shrink-0 text-muted-foreground"
                  aria-hidden="true"
                />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent
              side="top"
              align="end"
              className="w-[--radix-dropdown-menu-trigger-width]"
            >
              <DropdownMenuLabel className="font-normal">
                <span className="block truncate text-sm font-medium">
                  {displayName}
                </span>
                <span className="block truncate text-xs text-muted-foreground">
                  {user.username}
                </span>
              </DropdownMenuLabel>
              <DropdownMenuSeparator />
              <DropdownMenuItem>
                <UserRound className="size-4" aria-hidden="true" />
                {tNav("userMenu.profile")}
              </DropdownMenuItem>
              <DropdownMenuItem asChild>
                <Link href="/settings">
                  <Settings className="size-4" aria-hidden="true" />
                  {tNav("userMenu.settings")}
                </Link>
              </DropdownMenuItem>
              <DropdownMenuSeparator />
              {locales.map((targetLocale) => (
                <DropdownMenuItem
                  key={targetLocale}
                  disabled={targetLocale === locale}
                  onSelect={() => {
                    router.replace(currentPathname, { locale: targetLocale });
                  }}
                >
                  <Languages className="size-4" aria-hidden="true" />
                  {tCommon(`language.${targetLocale}`)}
                </DropdownMenuItem>
              ))}
              <DropdownMenuSeparator />
              <DropdownMenuItem
                className="text-destructive focus:text-destructive"
                onSelect={handleLogout}
              >
                <LogOut className="size-4" aria-hidden="true" />
                {tNav("userMenu.logout")}
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      </div>
    </aside>
  );
}
