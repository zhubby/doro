"use client";

import { AnimatePresence, motion, useReducedMotion } from "motion/react";

type PageTransitionProps = {
  children: React.ReactNode;
  pathname: string;
};

export function PageTransition({ children, pathname }: PageTransitionProps) {
  const prefersReducedMotion = useReducedMotion();

  if (prefersReducedMotion) {
    return <div className="flex min-h-0 flex-1 flex-col overflow-hidden">{children}</div>;
  }

  return (
    <AnimatePresence mode="wait" initial={false}>
      <motion.div
        key={pathname}
        className="flex min-h-0 flex-1 flex-col overflow-hidden"
        initial={{ opacity: 0, y: 8, filter: "blur(2px)" }}
        animate={{ opacity: 1, y: 0, filter: "blur(0px)" }}
        exit={{ opacity: 0, y: -6, filter: "blur(1px)" }}
        transition={{ duration: 0.18, ease: [0.22, 1, 0.36, 1] }}
      >
        {children}
      </motion.div>
    </AnimatePresence>
  );
}
