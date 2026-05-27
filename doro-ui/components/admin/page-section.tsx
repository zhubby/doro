import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { cn } from "@/lib/utils";

type PageSectionProps = {
  title?: string;
  description?: string;
  toolbar?: React.ReactNode;
  children: React.ReactNode;
  className?: string;
  contentClassName?: string;
};

export function PageSection({
  title,
  description,
  toolbar,
  children,
  className,
  contentClassName,
}: PageSectionProps) {
  return (
    <Card className={className}>
      {(title || description || toolbar) && (
        <CardHeader>
          <div className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
            <div>
              {title ? <CardTitle>{title}</CardTitle> : null}
              {description ? (
                <CardDescription>{description}</CardDescription>
              ) : null}
            </div>
            {toolbar ? <div className="flex flex-wrap gap-2">{toolbar}</div> : null}
          </div>
        </CardHeader>
      )}
      <CardContent className={cn(!title && !description && "pt-6", contentClassName)}>
        {children}
      </CardContent>
    </Card>
  );
}
