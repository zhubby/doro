"use client";

import { Plus, Trash2 } from "lucide-react";

import { Button } from "@/components/ui/button";

export type KeyValueRow = {
  id: string;
  key: string;
  value: string;
};

export type StringListRow = {
  id: string;
  value: string;
};

let repeatingFieldRowId = 0;

function nextRepeatingFieldRowId() {
  repeatingFieldRowId += 1;
  return `repeating-field-row-${repeatingFieldRowId}`;
}

function emptyKeyValueRow(): KeyValueRow {
  return {
    id: nextRepeatingFieldRowId(),
    key: "",
    value: "",
  };
}

function emptyStringListRow(): StringListRow {
  return {
    id: nextRepeatingFieldRowId(),
    value: "",
  };
}

export function KeyValueRowsField({
  label,
  rows,
  keyPlaceholder,
  valuePlaceholder,
  addLabel,
  emptyText,
  onChange,
}: {
  label: string;
  rows: KeyValueRow[];
  keyPlaceholder: string;
  valuePlaceholder: string;
  addLabel: string;
  emptyText: string;
  onChange: (rows: KeyValueRow[]) => void;
}) {
  const updateRow = (id: string, patch: Partial<KeyValueRow>) => {
    onChange(rows.map((row) => (row.id === id ? { ...row, ...patch } : row)));
  };

  return (
    <div className="space-y-3 rounded-md border bg-muted/10 p-3">
      <div className="flex items-center justify-between gap-3">
        <span className="text-sm font-medium">{label}</span>
        <Button
          type="button"
          variant="outline"
          size="sm"
          onClick={() => onChange([...rows, emptyKeyValueRow()])}
        >
          <Plus className="size-4" aria-hidden="true" />
          {addLabel}
        </Button>
      </div>

      {rows.length === 0 ? (
        <div className="rounded-md border border-dashed bg-background px-3 py-4 text-center text-sm text-muted-foreground">
          {emptyText}
        </div>
      ) : (
        <div className="space-y-2">
          {rows.map((row) => (
            <div
              key={row.id}
              className="grid gap-2 sm:grid-cols-[minmax(0,0.8fr)_minmax(0,1fr)_2.25rem]"
            >
              <input
                value={row.key}
                onChange={(event) => updateRow(row.id, { key: event.target.value })}
                placeholder={keyPlaceholder}
                aria-label={`${label}名称`}
                className="h-9 min-w-0 rounded-md border bg-background px-3 text-sm outline-none ring-offset-background focus-visible:ring-2 focus-visible:ring-ring"
              />
              <input
                value={row.value}
                onChange={(event) => updateRow(row.id, { value: event.target.value })}
                placeholder={valuePlaceholder}
                aria-label={`${label}值`}
                className="h-9 min-w-0 rounded-md border bg-background px-3 text-sm outline-none ring-offset-background focus-visible:ring-2 focus-visible:ring-ring"
              />
              <Button
                type="button"
                variant="outline"
                size="icon"
                aria-label={`删除${label}`}
                onClick={() => onChange(rows.filter((item) => item.id !== row.id))}
              >
                <Trash2 className="size-4" aria-hidden="true" />
              </Button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

export function StringListRowsField({
  label,
  rows,
  valuePlaceholder,
  addLabel,
  emptyText,
  onChange,
}: {
  label: string;
  rows: StringListRow[];
  valuePlaceholder: string;
  addLabel: string;
  emptyText: string;
  onChange: (rows: StringListRow[]) => void;
}) {
  const updateRow = (id: string, patch: Partial<StringListRow>) => {
    onChange(rows.map((row) => (row.id === id ? { ...row, ...patch } : row)));
  };

  return (
    <div className="space-y-3 rounded-md border bg-muted/10 p-3">
      <div className="flex items-center justify-between gap-3">
        <span className="text-sm font-medium">{label}</span>
        <Button
          type="button"
          variant="outline"
          size="sm"
          onClick={() => onChange([...rows, emptyStringListRow()])}
        >
          <Plus className="size-4" aria-hidden="true" />
          {addLabel}
        </Button>
      </div>

      {rows.length === 0 ? (
        <div className="rounded-md border border-dashed bg-background px-3 py-4 text-center text-sm text-muted-foreground">
          {emptyText}
        </div>
      ) : (
        <div className="space-y-2">
          {rows.map((row) => (
            <div key={row.id} className="grid gap-2 sm:grid-cols-[minmax(0,1fr)_2.25rem]">
              <input
                value={row.value}
                onChange={(event) => updateRow(row.id, { value: event.target.value })}
                placeholder={valuePlaceholder}
                aria-label={label}
                className="h-9 min-w-0 rounded-md border bg-background px-3 text-sm outline-none ring-offset-background focus-visible:ring-2 focus-visible:ring-ring"
              />
              <Button
                type="button"
                variant="outline"
                size="icon"
                aria-label={`删除${label}`}
                onClick={() => onChange(rows.filter((item) => item.id !== row.id))}
              >
                <Trash2 className="size-4" aria-hidden="true" />
              </Button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
