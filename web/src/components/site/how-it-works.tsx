"use client";

import { motion } from "framer-motion";
import {
  RadioTower,
  KeyRound,
  PlugZap,
  Lock,
  MonitorSmartphone,
} from "lucide-react";
import type { ReactNode } from "react";
import { Reveal } from "@/components/ui/reveal";

function Device({
  icon,
  label,
  sub,
}: {
  icon: ReactNode;
  label: string;
  sub: string;
}) {
  return (
    <div className="glass-strong relative z-10 flex w-36 shrink-0 flex-col items-center gap-2 rounded-2xl p-5 text-center sm:w-40">
      <div className="grid h-12 w-12 place-items-center rounded-xl bg-[color:var(--color-accent-dim)] ring-1 ring-accent/20">
        {icon}
      </div>
      <div className="text-sm font-semibold">{label}</div>
      <div className="text-[11px] leading-tight text-faint">{sub}</div>
    </div>
  );
}

function Link() {
  return (
    <div className="relative mx-2 hidden h-px flex-1 items-center sm:flex">
      <div className="h-px w-full bg-gradient-to-r from-accent/10 via-accent/50 to-accent/10" />
      {/* travelling packets */}
      {[0, 1, 2].map((i) => (
        <motion.span
          key={i}
          className="absolute top-1/2 h-2 w-2 -translate-y-1/2 rounded-full bg-accent shadow-[0_0_12px_2px_rgba(52,211,153,0.7)]"
          initial={{ left: "0%" }}
          animate={{ left: "100%" }}
          transition={{
            duration: 2,
            repeat: Infinity,
            delay: i * 0.66,
            ease: "linear",
          }}
        />
      ))}
      {/* lock badge */}
      <div className="glass absolute left-1/2 top-1/2 flex -translate-x-1/2 -translate-y-1/2 items-center gap-1 rounded-full px-2.5 py-1 text-[10px] text-muted">
        <Lock className="h-3 w-3 text-accent" />
        TLS 1.3
      </div>
    </div>
  );
}

const steps = [
  {
    n: "01",
    Icon: RadioTower,
    title: "Host",
    desc: "Open MyVPN on your home device, name your network, and click Host. Your connection is now a private VPN.",
  },
  {
    n: "02",
    Icon: KeyRound,
    title: "Pair",
    desc: "Same Wi‑Fi devices discover the host automatically. From afar, share the host's pairing code (plus an optional passphrase).",
  },
  {
    n: "03",
    Icon: PlugZap,
    title: "Connect",
    desc: "The second device routes all of its traffic through the host over an encrypted, hole-punched P2P tunnel.",
  },
];

export function HowItWorks() {
  return (
    <section id="how" className="px-4 py-24">
      <div className="mx-auto max-w-5xl">
        <Reveal className="mx-auto max-w-2xl text-center">
          <div className="mb-3 text-sm font-medium text-accent">How it works</div>
          <h2 className="text-balance text-3xl font-semibold tracking-tight sm:text-4xl">
            Two devices. One encrypted tunnel.
          </h2>
          <p className="mx-auto mt-4 max-w-lg text-pretty text-[15px] leading-relaxed text-muted">
            No accounts, no relays you have to trust with your traffic. Just a
            direct, private link between machines you own.
          </p>
        </Reveal>

        <Reveal delay={0.1}>
          <div className="mt-14 flex items-center justify-center gap-3 rounded-3xl">
            <Device
              icon={<RadioTower className="h-6 w-6 text-accent" />}
              label="Host"
              sub="Shares its network"
            />
            <Link />
            <Device
              icon={<MonitorSmartphone className="h-6 w-6 text-accent" />}
              label="Connect"
              sub="Routes all traffic"
            />
          </div>
        </Reveal>

        <div className="mt-14 grid gap-4 md:grid-cols-3">
          {steps.map((s, i) => (
            <Reveal key={s.title} delay={i * 0.1}>
              <div className="glass h-full rounded-2xl p-6">
                <div className="flex items-center justify-between">
                  <div className="grid h-10 w-10 place-items-center rounded-xl bg-[color:var(--color-accent-dim)] ring-1 ring-accent/20">
                    <s.Icon className="h-5 w-5 text-accent" />
                  </div>
                  <span className="font-mono text-2xl font-semibold text-white/10">
                    {s.n}
                  </span>
                </div>
                <h3 className="mt-4 text-base font-semibold">{s.title}</h3>
                <p className="mt-1.5 text-sm leading-relaxed text-muted">
                  {s.desc}
                </p>
              </div>
            </Reveal>
          ))}
        </div>
      </div>
    </section>
  );
}
