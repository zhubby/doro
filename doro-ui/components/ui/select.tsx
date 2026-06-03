"use client";

import * as React from "react";
import { ChevronDown } from "lucide-react";

import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { cn } from "@/lib/utils";

export type SelectOption = {
  value: string;
  label: React.ReactNode;
  disabled?: boolean;
};

type SelectProps = Omit<
  React.ButtonHTMLAttributes<HTMLButtonElement>,
  "onChange" | "type" | "value"
> & {
  value: string;
  onValueChange: (value: string) => void;
  options: SelectOption[];
  placeholder?: React.ReactNode;
  contentClassName?: string;
  align?: "start" | "center" | "end";
  required?: boolean;
};

const Select = React.forwardRef<HTMLButtonElement, SelectProps>(
  (
    {
      className,
      contentClassName,
      value,
      onValueChange,
      options,
      placeholder,
      disabled,
      required,
      align = "start",
      ...props
    },
    ref,
  ) => {
    const selectedOption = options.find((option) => option.value === value);
    const selectedLabel = selectedOption?.label ?? placeholder;

    return (
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <button
            ref={ref}
            type="button"
            className={cn(
              "flex h-10 w-full items-center justify-between gap-2 rounded-md border bg-background px-3 text-left text-sm outline-none ring-offset-background transition-colors hover:bg-accent/50 focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-60",
              className,
            )}
            disabled={disabled}
            aria-required={required || undefined}
            {...props}
          >
            <span
              className={cn(
                "min-w-0 flex-1 truncate",
                !selectedLabel && "text-muted-foreground",
              )}
            >
              {selectedLabel ?? ""}
            </span>
            <ChevronDown
              className="size-4 shrink-0 text-muted-foreground"
              aria-hidden="true"
            />
          </button>
        </DropdownMenuTrigger>
        <DropdownMenuContent
          align={align}
          className={cn(
            "max-h-72 w-[--radix-dropdown-menu-trigger-width] min-w-[--radix-dropdown-menu-trigger-width] overflow-y-auto",
            contentClassName,
          )}
        >
          <DropdownMenuRadioGroup value={value} onValueChange={onValueChange}>
            {options.map((option) => (
              <DropdownMenuRadioItem
                key={option.value}
                value={option.value}
                disabled={option.disabled}
                className="pr-8"
              >
                <span className="truncate">{option.label}</span>
              </DropdownMenuRadioItem>
            ))}
          </DropdownMenuRadioGroup>
        </DropdownMenuContent>
      </DropdownMenu>
    );
  },
);
Select.displayName = "Select";

export { Select };
