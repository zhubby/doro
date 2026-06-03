"use client";

import * as React from "react";

import { cn } from "@/lib/utils";

export type SwitchProps = Omit<
  React.ButtonHTMLAttributes<HTMLButtonElement>,
  "checked" | "defaultChecked" | "onChange"
> & {
  checked?: boolean;
  defaultChecked?: boolean;
  onCheckedChange?: (checked: boolean) => void;
};

const Switch = React.forwardRef<HTMLButtonElement, SwitchProps>(
  (
    {
      className,
      checked,
      defaultChecked = false,
      disabled,
      onCheckedChange,
      onClick,
      ...props
    },
    ref,
  ) => {
    const [uncontrolledChecked, setUncontrolledChecked] =
      React.useState(defaultChecked);
    const isControlled = checked !== undefined;
    const isChecked = isControlled ? checked : uncontrolledChecked;

    function handleClick(event: React.MouseEvent<HTMLButtonElement>) {
      onClick?.(event);
      if (event.defaultPrevented || disabled) {
        return;
      }

      const nextChecked = !isChecked;
      if (!isControlled) {
        setUncontrolledChecked(nextChecked);
      }
      onCheckedChange?.(nextChecked);
    }

    return (
      <button
        type="button"
        role="switch"
        aria-checked={isChecked}
        data-state={isChecked ? "checked" : "unchecked"}
        disabled={disabled}
        ref={ref}
        onClick={handleClick}
        className={cn(
          "inline-flex h-6 w-11 shrink-0 items-center rounded-full border border-input bg-muted p-0.5 shadow-inner transition-colors duration-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50",
          isChecked && "border-primary/60 bg-primary",
          className,
        )}
        {...props}
      >
        <span
          className={cn(
            "pointer-events-none flex size-5 items-center justify-center rounded-full bg-background text-primary shadow-sm transition-transform duration-200",
            isChecked ? "translate-x-5" : "translate-x-0",
          )}
        >
          <span
            className={cn(
              "size-1.5 rounded-full bg-muted-foreground/50 transition-colors",
              isChecked && "bg-primary",
            )}
          />
        </span>
      </button>
    );
  },
);
Switch.displayName = "Switch";

export { Switch };
