"use client";

import { useTranslations } from "next-intl";

import { Button } from "@/components/ui/button";
import {
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from "@/components/ui/command";
import { useRouter } from "@/i18n/navigation";
import { navigation } from "@/lib/navigation";
import { Search } from "lucide-react";

type SearchCommandProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
};

export function SearchCommand({ open, onOpenChange }: SearchCommandProps) {
  const router = useRouter();
  const t = useTranslations("navigation.search");
  const tNav = useTranslations("navigation");

  return (
    <>
      <Button
        variant="outline"
        size="icon"
        onClick={() => onOpenChange(true)}
        aria-label={t("open")}
        title={t("open")}
      >
        <Search className="size-4" aria-hidden="true" />
      </Button>
      <CommandDialog
        open={open}
        onOpenChange={onOpenChange}
        title={t("title")}
      >
        <CommandInput placeholder={t("placeholder")} />
        <CommandList>
          <CommandEmpty>{t("empty")}</CommandEmpty>
          <CommandGroup heading={t("group")}>
            {navigation.map((item) => {
              const Icon = item.icon;
              const label = tNav(`items.${item.id}.label`);
              const description = tNav(`items.${item.id}.description`);

              return (
                <CommandItem
                  key={item.href}
                  value={`${label} ${description}`}
                  onSelect={() => {
                    onOpenChange(false);
                    router.push(item.href);
                  }}
                >
                  <Icon className="size-4" aria-hidden="true" />
                  <span>{label}</span>
                  <span className="ml-auto max-w-48 truncate text-xs text-muted-foreground">
                    {description}
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
