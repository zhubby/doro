"use client";

import { Toaster } from "sonner";

export function AppToaster() {
  return (
    <Toaster
      duration={3600}
      position="top-center"
      toastOptions={{
        classNames: {
          toast:
            "min-h-11 rounded-md border border-border bg-popover px-4 py-3 text-popover-foreground shadow-lg shadow-black/5 dark:shadow-black/30",
          title: "text-sm font-medium leading-5",
          description: "text-sm text-muted-foreground",
          icon: "text-muted-foreground",
          success: "border-l-4 border-l-primary",
          info: "border-l-4 border-l-primary",
          warning: "border-l-4 border-l-ring",
          error: "border-l-4 border-l-destructive",
        },
      }}
    />
  );
}
