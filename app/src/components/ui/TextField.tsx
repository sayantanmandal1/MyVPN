import type { InputHTMLAttributes, ReactNode } from "react";
import { cn } from "../../lib/utils";

interface Props extends InputHTMLAttributes<HTMLInputElement> {
  label?: string;
  hint?: ReactNode;
}

export function TextField({ label, hint, className, ...props }: Props) {
  return (
    <label className="block">
      {label && (
        <span className="mb-1.5 block text-xs font-medium tracking-wide text-[color:var(--color-muted)] uppercase">
          {label}
        </span>
      )}
      <input
        className={cn(
          "glass-inset w-full select-text rounded-xl px-4 py-3 text-sm text-ink outline-none transition",
          "focus:ring-2 focus:ring-accent/40",
          className,
        )}
        {...props}
      />
      {hint && (
        <span className="mt-1.5 block text-xs text-[color:var(--color-faint)]">
          {hint}
        </span>
      )}
    </label>
  );
}
