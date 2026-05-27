import { cn } from "@/lib/utils";

type ToolbarProps = {
  left?: React.ReactNode;
  right?: React.ReactNode;
  className?: string;
};

export function Toolbar({ left, right, className }: ToolbarProps) {
  return (
    <div
      className={cn(
        "flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between",
        className,
      )}
    >
      <div className="flex flex-wrap gap-2">{left}</div>
      <div className="flex flex-wrap gap-2">{right}</div>
    </div>
  );
}
