import type { ReactNode } from "react";
import { motion, type HTMLMotionProps } from "motion/react";

import { PRESS } from "@/lib/motion";
import { cn } from "@/lib/cn";

type Variant = "primary" | "surface" | "ghost" | "danger";
type Size = "sm" | "md" | "lg";

/**
 * Buttons are ink, never blue. Blue is reserved for voice, so a filled blue
 * button would break the one rule the palette runs on.
 */
const VARIANT: Record<Variant, string> = {
  primary: "bg-ink text-paper font-medium hover:opacity-85",
  surface:
    "bg-card border border-line-2 text-ink-2 hover:bg-sunken hover:text-ink",
  ghost: "text-ink-3 hover:bg-sunken hover:text-ink",
  danger: "bg-stop-tint border border-stop/25 text-stop hover:bg-stop/10",
};

const SIZE: Record<Size, string> = {
  sm: "h-7 px-2.5 text-[12px] rounded-lg gap-1.5",
  md: "h-9 px-3.5 text-[13px] rounded-ctl gap-2",
  lg: "h-10 px-4 text-[13.5px] rounded-ctl gap-2",
};

interface Props extends Omit<HTMLMotionProps<"button">, "ref" | "children"> {
  children?: ReactNode;
  variant?: Variant;
  size?: Size;
  icon?: ReactNode;
}

export function Button({
  variant = "surface",
  size = "md",
  icon,
  className,
  children,
  ...rest
}: Props) {
  return (
    <motion.button
      type="button"
      {...PRESS}
      className={cn(
        "no-drag inline-flex items-center justify-center whitespace-nowrap",
        "transition-colors duration-150",
        "disabled:pointer-events-none disabled:opacity-40",
        VARIANT[variant],
        SIZE[size],
        className,
      )}
      {...rest}
    >
      {icon}
      {children}
    </motion.button>
  );
}
