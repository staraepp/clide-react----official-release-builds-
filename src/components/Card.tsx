import type { ReactNode } from "react";
import { motion } from "motion/react";

import { EASE } from "@/lib/motion";
import { cn } from "@/lib/cn";

/**
 * A section of the home screen.
 *
 * Deliberately flat: no border, fill, shadow or hover lift. Sections are
 * separated by hairlines and spacing from their parent, which is what keeps
 * the page reading as one document rather than a grid of panels.
 */
interface CardProps {
  children: ReactNode;
  className?: string;
  /** Stagger index for the entrance animation. */
  index?: number;
}

export function Card({ children, className, index = 0 }: CardProps) {
  return (
    <motion.section
      initial={{ opacity: 0, y: 6 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ ...EASE, delay: Math.min(index, 6) * 0.03 }}
      className={cn("min-w-0", className)}
    >
      {children}
    </motion.section>
  );
}

export function CardHeader({
  label,
  action,
}: {
  label: string;
  action?: ReactNode;
}) {
  return (
    <header className="flex h-5 items-center justify-between gap-3">
      <h2 className="label">{label}</h2>
      {action}
    </header>
  );
}
