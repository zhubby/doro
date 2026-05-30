"use client";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Link, usePathname } from "@/i18n/navigation";

export type ModuleTab = {
  label: string;
  href: string;
  count?: number;
};

export function ModuleTabs({
  tabs,
  action,
}: {
  tabs: ModuleTab[];
  action?: React.ReactNode;
}) {
  const pathname = usePathname();

  return (
    <Card className="shadow-none">
      <div className="flex flex-col gap-3 p-2 sm:flex-row sm:items-center sm:justify-between">
        <div className="flex flex-wrap gap-1">
          {tabs.map((tab) => {
            const isActive =
              pathname === tab.href || pathname.startsWith(`${tab.href}/`);

            return (
              <Button
                key={tab.href}
                asChild
                variant={isActive ? "secondary" : "ghost"}
                className="justify-start"
              >
                <Link href={tab.href}>
                  {tab.label}
                  {tab.count ? (
                    <Badge variant="outline" className="ml-1">
                      {tab.count}
                    </Badge>
                  ) : null}
                </Link>
              </Button>
            );
          })}
        </div>
        {action ? <div className="flex flex-wrap gap-2">{action}</div> : null}
      </div>
    </Card>
  );
}
