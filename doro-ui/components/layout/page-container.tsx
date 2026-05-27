import { cn } from "@/lib/utils";

type PageContainerProps = {
  children: React.ReactNode;
  aside?: React.ReactNode;
  className?: string;
};

export function PageContainer({
  children,
  aside,
  className,
}: PageContainerProps) {
  if (!aside) {
    return <div className={cn("flex-1 space-y-6 p-6", className)}>{children}</div>;
  }

  return (
    <div
      className={cn(
        "grid flex-1 gap-6 p-6 xl:grid-cols-[1fr_22rem]",
        className,
      )}
    >
      <section className="min-w-0 space-y-6">{children}</section>
      <aside className="space-y-6">{aside}</aside>
    </div>
  );
}
