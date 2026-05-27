"use client";

import { useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type { SettingOption } from "@/types/dashboard";

export function SettingList({ settings }: { settings: SettingOption[] }) {
  const [values, setValues] = useState(
    () => new Map(settings.map((setting) => [setting.id, setting.value])),
  );

  return (
    <div className="divide-y rounded-lg border">
      {settings.map((setting) => {
        const value = values.get(setting.id) ?? setting.value;

        return (
          <div
            key={setting.id}
            className="grid gap-4 p-4 md:grid-cols-[10rem_1fr_auto] md:items-center"
          >
            <div>
              <p className="text-sm font-medium">{setting.label}</p>
              {setting.helper ? (
                <p className="mt-1 text-xs text-muted-foreground md:hidden">
                  {setting.helper}
                </p>
              ) : null}
            </div>
            <div>
              <div className="flex flex-wrap items-center gap-2">
                <Badge variant="secondary">{value}</Badge>
                {setting.choices?.map((choice) => (
                  <Button
                    key={choice}
                    type="button"
                    size="sm"
                    variant={choice === value ? "default" : "outline"}
                    onClick={() =>
                      setValues((current) => new Map(current).set(setting.id, choice))
                    }
                  >
                    {choice}
                  </Button>
                ))}
              </div>
              {setting.helper ? (
                <p className="mt-2 hidden text-xs text-muted-foreground md:block">
                  {setting.helper}
                </p>
              ) : null}
            </div>
            {setting.action ? (
              <Button variant="outline" size="sm">
                {setting.action}
              </Button>
            ) : null}
          </div>
        );
      })}
    </div>
  );
}
