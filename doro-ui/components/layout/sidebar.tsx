"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import {
  Boxes,
  ChevronUp,
  LogOut,
  Settings,
  UserRound,
} from "lucide-react";

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
            <p className="text-sm font-semibold">Doro Panel</p>
            <p className="text-xs text-muted-foreground">本地控制台</p>
          </div>
        </div>
        <ScrollArea className="min-h-0 flex-1 px-3 py-4">
          <nav className="grid gap-1" aria-label="控制面板导航">
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
                    <span>{item.label}</span>
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
                aria-label="打开用户菜单"
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
                Profile
              </DropdownMenuItem>
              <DropdownMenuItem asChild>
                <Link href="/settings">
                  <Settings className="size-4" aria-hidden="true" />
                  Settings
                </Link>
              </DropdownMenuItem>
              <DropdownMenuSeparator />
              <DropdownMenuItem
                className="text-destructive focus:text-destructive"
                onSelect={handleLogout}
              >
                <LogOut className="size-4" aria-hidden="true" />
                Logout
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      </div>
    </aside>
  );
}
