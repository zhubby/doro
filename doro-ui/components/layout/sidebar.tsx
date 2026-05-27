"use client";

import Link from "next/link";
import { Boxes, ShieldCheck } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import { navigation } from "@/lib/navigation";
import { cn } from "@/lib/utils";

export function Sidebar({ pathname }: { pathname: string }) {
  return (
    <aside className="border-b bg-card lg:border-b-0 lg:border-r">
      <div className="flex h-full flex-col">
        <div className="flex h-16 items-center gap-3 px-6">
          <div className="flex size-9 items-center justify-center rounded-lg bg-primary text-primary-foreground">
            <Boxes className="size-4" aria-hidden="true" />
          </div>
          <div>
            <p className="text-sm font-semibold">Doro Panel</p>
            <p className="text-xs text-muted-foreground">本地控制台</p>
          </div>
        </div>
        <Separator />
        <ScrollArea className="flex-1 px-3 py-4">
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
          <Card className="shadow-none">
            <CardHeader className="p-4 pb-2">
              <CardTitle className="text-sm">入口状态</CardTitle>
              <CardDescription>安全入口已启用</CardDescription>
            </CardHeader>
            <CardContent className="flex items-center gap-2 p-4 pt-0">
              <ShieldCheck
                className="size-4 text-primary"
                aria-hidden="true"
              />
              <span className="text-xs text-muted-foreground">
                v2.1.13-alpha.2
              </span>
            </CardContent>
          </Card>
        </div>
      </div>
    </aside>
  );
}
