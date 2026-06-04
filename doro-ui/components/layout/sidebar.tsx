"use client";

import {
  ChevronDown,
  ChevronUp,
  LogOut,
  Settings,
  UserRound,
} from "lucide-react";
import Image from "next/image";
import { useState } from "react";
import { useTranslations } from "next-intl";

import { AccountSettingsDialog } from "@/components/layout/account-settings-dialog";
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
import { Link, useRouter } from "@/i18n/navigation";
import { logout } from "@/lib/control-plane-api";
import { navigation } from "@/lib/navigation";
import { cn } from "@/lib/utils";
import type { UserSummary } from "@/types/api";

type AccountDialogTab = "profile" | "password" | "preferences";

const collapsibleNavigationItemIds = new Set(["virtualMachines", "docker"]);

export function Sidebar({
  pathname,
  user,
  onUserChange,
}: {
  pathname: string;
  user: UserSummary;
  onUserChange: (user: UserSummary) => void;
}) {
  const router = useRouter();
  const tCommon = useTranslations("common");
  const tNav = useTranslations("navigation");
  const [accountDialogOpen, setAccountDialogOpen] = useState(false);
  const [accountDialogTab, setAccountDialogTab] =
    useState<AccountDialogTab>("profile");
  const [collapsedItems, setCollapsedItems] = useState<Set<string>>(
    () => new Set(),
  );
  const displayName = user.display_name || user.username;
  const initials = displayName.trim().slice(0, 1).toUpperCase();

  function openAccountDialog(tab: AccountDialogTab) {
    setAccountDialogTab(tab);
    setAccountDialogOpen(true);
  }

  function toggleCollapsedItem(itemId: string) {
    setCollapsedItems((current) => {
      const next = new Set(current);

      if (next.has(itemId)) {
        next.delete(itemId);
      } else {
        next.add(itemId);
      }

      return next;
    });
  }

  async function handleLogout() {
    await logout();
    router.replace("/login");
  }

  return (
    <aside className="min-h-0 border-b bg-card lg:border-b-0 lg:border-r">
      <div className="flex h-full min-h-0 flex-col">
        <div className="flex h-20 shrink-0 items-center gap-3 border-b px-6">
          <div className="flex size-11 items-center justify-center rounded-lg bg-accent shadow-sm ring-1 ring-primary/15">
            <Image
              src="/brand/doro-logo.png"
              alt=""
              width={40}
              height={40}
              className="size-10 object-contain"
              priority
              aria-hidden="true"
            />
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
              const isActive = navigationItemActive(item.href, pathname);
              const childActive = item.children?.some((child) =>
                navigationItemActive(child.href, pathname),
              );
              const canCollapse =
                Boolean(item.children?.length) &&
                collapsibleNavigationItemIds.has(item.id);
              const isCollapsed = canCollapse && collapsedItems.has(item.id);
              const submenuId = `sidebar-subnav-${item.id}`;
              const label = tNav(`items.${item.id}.label`);

              return (
                <div key={item.href} className="grid gap-1">
                  {canCollapse ? (
                    <Button
                      type="button"
                      variant="ghost"
                      className="w-full justify-start"
                      aria-controls={submenuId}
                      aria-expanded={!isCollapsed}
                      aria-label={tNav(
                        isCollapsed
                          ? "groupToggle.expand"
                          : "groupToggle.collapse",
                        { label },
                      )}
                      onClick={() => toggleCollapsedItem(item.id)}
                    >
                      <Icon className="size-4" aria-hidden="true" />
                      <span className="truncate">{label}</span>
                      {item.count ? (
                        <Badge variant="outline" className="ml-auto">
                          {item.count}
                        </Badge>
                      ) : null}
                      <ChevronDown
                        className={cn(
                          "ml-auto size-4 text-muted-foreground transition-transform",
                          item.count && "ml-0",
                          isCollapsed && "-rotate-90",
                        )}
                        aria-hidden="true"
                      />
                    </Button>
                  ) : (
                    <Button
                      asChild
                      variant={isActive || childActive ? "secondary" : "ghost"}
                      className={cn(
                        "justify-start",
                        (isActive || childActive) && "font-semibold",
                      )}
                    >
                      <Link href={item.href}>
                        <Icon className="size-4" aria-hidden="true" />
                        <span className="truncate">{label}</span>
                        {item.count ? (
                          <Badge variant="outline" className="ml-auto">
                            {item.count}
                          </Badge>
                        ) : null}
                      </Link>
                    </Button>
                  )}
                  {item.children?.length ? (
                    <div
                      id={submenuId}
                      className={cn(
                        "ml-4 gap-1 border-l pl-2",
                        isCollapsed ? "hidden" : "grid",
                      )}
                      aria-hidden={isCollapsed}
                    >
                      {item.children.map((child) => {
                        const ChildIcon = child.icon;
                        const isChildActive = navigationItemActive(
                          child.href,
                          pathname,
                        );

                        return (
                          <Button
                            key={child.href}
                            asChild
                            variant={isChildActive ? "secondary" : "ghost"}
                            size="sm"
                            className={cn(
                              "h-8 justify-start text-xs",
                              isChildActive && "font-semibold",
                            )}
                          >
                            <Link href={child.href}>
                              <ChildIcon className="size-3.5" aria-hidden="true" />
                              <span>{tNav(`items.${child.id}.label`)}</span>
                            </Link>
                          </Button>
                        );
                      })}
                    </div>
                  ) : null}
                </div>
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
              <DropdownMenuItem onSelect={() => openAccountDialog("profile")}>
                <UserRound className="size-4" aria-hidden="true" />
                {tNav("userMenu.profile")}
              </DropdownMenuItem>
              <DropdownMenuItem onSelect={() => openAccountDialog("preferences")}>
                <Settings className="size-4" aria-hidden="true" />
                {tNav("userMenu.settings")}
              </DropdownMenuItem>
              <DropdownMenuSeparator />
              <DropdownMenuItem
                className="text-red-600 focus:bg-red-50 focus:text-red-700 dark:text-red-300 dark:focus:bg-red-950/40 dark:focus:text-red-200"
                onSelect={handleLogout}
              >
                <LogOut className="size-4" aria-hidden="true" />
                {tNav("userMenu.logout")}
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
          <AccountSettingsDialog
            open={accountDialogOpen}
            onOpenChange={setAccountDialogOpen}
            initialTab={accountDialogTab}
            user={user}
            onUserChange={onUserChange}
          />
        </div>
      </div>
    </aside>
  );
}

function navigationItemActive(href: string, pathname: string) {
  return href === "/"
    ? pathname === "/"
    : pathname === href || pathname.startsWith(`${href}/`);
}
