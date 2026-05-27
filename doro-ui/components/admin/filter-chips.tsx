"use client";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";

export type FilterChip = {
  label: string;
  value: string;
  count?: number;
};

type FilterChipsProps = {
  filters: FilterChip[];
  value: string;
  onValueChange: (value: string) => void;
};

export function FilterChips({
  filters,
  value,
  onValueChange,
}: FilterChipsProps) {
  return (
    <div className="flex flex-wrap gap-2">
      {filters.map((filter) => {
        const isActive = filter.value === value;

        return (
          <Button
            key={filter.value}
            type="button"
            variant={isActive ? "default" : "outline"}
            size="sm"
            onClick={() => onValueChange(filter.value)}
          >
            {filter.label}
            {typeof filter.count === "number" ? (
              <Badge
                variant={isActive ? "secondary" : "outline"}
                className="ml-1"
              >
                {filter.count}
              </Badge>
            ) : null}
          </Button>
        );
      })}
    </div>
  );
}
