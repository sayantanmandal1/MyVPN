import type { ButtonHTMLAttributes } from "react";
import { cn } from "../../lib/utils";

type Variant = "primary" | "ghost" | "danger" | "subtle";
type Size = "sm" | "md" | "lg";

interface Props extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
  size?: Size;
}

const variants: Record<Variant, string> = {
  primary:
    "bg-accent text-black hover:bg-[color:var(--color-accent-strong)] shadow-[0_10px_34px_-10px_rgba(52,211,153,0.7)]",
  danger:
    "bg-[color:var(--color-danger)]/15 text-[color:var(--color-danger)] border border-[color:var(--color-danger)]/30 hover:bg-[color:var(--color-danger)]/25",
  ghost: "text-[color:var(--color-muted)] hover:text-ink hover:bg-white/5",
  subtle: "glass text-ink hover:bg-white/10",
};

const sizes: Record<Size, string> = {
  sm: "px-3 py-1.5 text-xs",
  md: "px-4 py-2.5 text-sm",
  lg: "px-5 py-3.5 text-[15px]",
};

export function Button({
  variant = "subtle",
  size = "md",
  className,
  ...props
}: Props) {
  return (
    <button
      className={cn(
        "inline-flex items-center justify-center gap-2 rounded-xl font-medium transition-all duration-200",
        "active:scale-[0.98] disabled:cursor-not-allowed disabled:opacity-40 disabled:active:scale-100",
        variants[variant],
        sizes[size],
        className,
      )}
      {...props}
    />
  );
}
