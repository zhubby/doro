"use client";

import { useRouter } from "next/navigation";

import { Button } from "@/components/ui/button";
import {
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from "@/components/ui/command";
import { navigation } from "@/lib/navigation";
import { Search } from "lucide-react";

type SearchCommandProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
};

export function SearchCommand({ open, onOpenChange }: SearchCommandProps) {
  const router = useRouter();

  return (
    <>
      <Button
        variant="outline"
        size="icon"
        onClick={() => onOpenChange(true)}
        aria-label="打开搜索面板"
        title="搜索"
      >
        <Search className="size-4" aria-hidden="true" />
      </Button>
      <CommandDialog
        open={open}
        onOpenChange={onOpenChange}
        title="搜索导航"
      >
        <CommandInput placeholder="搜索页面、模块或能力..." />
        <CommandList>
          <CommandEmpty>未找到匹配结果。</CommandEmpty>
          <CommandGroup heading="导航">
            {navigation.map((item) => {
              const Icon = item.icon;

              return (
                <CommandItem
                  key={item.href}
                  value={`${item.label} ${item.description}`}
                  onSelect={() => {
                    onOpenChange(false);
                    router.push(item.href);
                  }}
                >
                  <Icon className="size-4" aria-hidden="true" />
                  <span>{item.label}</span>
                  <span className="ml-auto max-w-48 truncate text-xs text-muted-foreground">
                    {item.description}
                  </span>
                </CommandItem>
              );
            })}
          </CommandGroup>
        </CommandList>
      </CommandDialog>
    </>
  );
}
