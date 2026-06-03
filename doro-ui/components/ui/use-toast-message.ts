"use client";

import { useEffect, useRef } from "react";
import { toast } from "sonner";

type ToastKind = "error" | "info" | "success" | "warning";

type UseToastMessageOptions = {
  id?: string;
  kind?: ToastKind;
  prefix?: string;
};

export function useToastMessage(
  message: string | null | undefined,
  { id, kind = "info", prefix }: UseToastMessageOptions = {},
) {
  const lastMessageRef = useRef<string | null>(null);

  useEffect(() => {
    const normalizedMessage = message?.trim();
    if (!normalizedMessage) {
      lastMessageRef.current = null;
      return;
    }

    const toastMessage = prefix ? `${prefix}${normalizedMessage}` : normalizedMessage;
    if (lastMessageRef.current === toastMessage) {
      return;
    }
    lastMessageRef.current = toastMessage;

    const options = id ? { id } : undefined;
    if (kind === "error") {
      toast.error(toastMessage, options);
    } else if (kind === "success") {
      toast.success(toastMessage, options);
    } else if (kind === "warning") {
      toast.warning(toastMessage, options);
    } else {
      toast.info(toastMessage, options);
    }
  }, [id, kind, message, prefix]);
}

export function ToastMessage({
  message,
  ...options
}: {
  message: string | null | undefined;
} & UseToastMessageOptions) {
  useToastMessage(message, options);
  return null;
}
