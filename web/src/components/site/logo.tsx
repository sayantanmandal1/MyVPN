import { cn } from "@/lib/utils";

export function Logo({ className }: { className?: string }) {
  return (
    <span
      className={cn(
        "grid place-items-center rounded-lg bg-[color:var(--color-accent-dim)] ring-1 ring-[color:var(--color-accent)]/30",
        className,
      )}
    >
      <svg viewBox="0 0 24 24" className="h-[58%] w-[58%] text-accent" fill="none">
        <path
          d="M12 2 4 5v6c0 5 3.5 8.5 8 11 4.5-2.5 8-6 8-11V5l-8-3Z"
          stroke="currentColor"
          strokeWidth="1.6"
          strokeLinejoin="round"
        />
        <path
          d="m8.5 12 2.5 2.5 4.5-5"
          stroke="currentColor"
          strokeWidth="1.6"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      </svg>
    </span>
  );
}
