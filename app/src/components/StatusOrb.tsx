import { motion } from "framer-motion";
import { ShieldCheck, Loader2, Power, RadioTower } from "lucide-react";
import type { ConnectionState, VpnRole } from "../lib/api";
import { cn } from "../lib/utils";

export function StatusOrb({
  state,
  role,
}: {
  state: ConnectionState;
  role: VpnRole;
}) {
  const active = state === "connected" || state === "hosting";
  const busy =
    state === "connecting" ||
    state === "discovering" ||
    state === "reconnecting";
  const error = state === "error";

  const Icon = error
    ? Power
    : busy
      ? Loader2
      : role === "host"
        ? RadioTower
        : ShieldCheck;

  const accent = active
    ? "var(--color-accent)"
    : error
      ? "var(--color-danger)"
      : busy
        ? "var(--color-warn)"
        : "rgba(255,255,255,0.35)";

  return (
    <div className="relative grid h-56 w-56 place-items-center">
      {/* pulse rings */}
      {active &&
        [0, 1, 2].map((i) => (
          <motion.span
            key={i}
            className="absolute rounded-full border"
            style={{ borderColor: accent }}
            initial={{ width: 120, height: 120, opacity: 0.5 }}
            animate={{ width: 224, height: 224, opacity: 0 }}
            transition={{
              duration: 2.6,
              repeat: Infinity,
              delay: i * 0.85,
              ease: "easeOut",
            }}
          />
        ))}

      {/* outer glass ring */}
      <div
        className="absolute h-44 w-44 rounded-full"
        style={{
          background: `radial-gradient(circle at 50% 40%, ${accent}22, transparent 65%)`,
        }}
      />
      <div className="glass-strong absolute h-44 w-44 rounded-full" />

      {/* core */}
      <motion.div
        className={cn(
          "relative grid h-32 w-32 place-items-center rounded-full",
          active && "ring-accent",
        )}
        style={{
          background:
            "linear-gradient(180deg, rgba(255,255,255,0.08), rgba(255,255,255,0.02))",
          border: "1px solid rgba(255,255,255,0.12)",
        }}
        animate={active ? { scale: [1, 1.03, 1] } : { scale: 1 }}
        transition={{ duration: 3, repeat: Infinity, ease: "easeInOut" }}
      >
        <Icon
          className={cn("h-12 w-12", busy && "animate-spin")}
          style={{ color: accent }}
          strokeWidth={1.5}
        />
      </motion.div>
    </div>
  );
}
