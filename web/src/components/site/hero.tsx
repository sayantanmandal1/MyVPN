"use client";

import { motion } from "framer-motion";
import { Download, ShieldCheck, ArrowDown, Wifi } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Logo } from "@/components/site/logo";
import { GithubMark } from "@/components/site/icons";
import { site } from "@/lib/site";

function FloatingOrbs() {
  return (
    <div className="pointer-events-none absolute inset-0 -z-10 overflow-hidden">
      <div
        className="absolute -left-20 top-10 h-[420px] w-[420px] rounded-full opacity-50 blur-[90px]"
        style={{
          background:
            "radial-gradient(circle, rgba(52,211,153,0.35), transparent 65%)",
          animation: "float-slow 9s ease-in-out infinite",
        }}
      />
      <div
        className="absolute -right-16 top-32 h-[380px] w-[380px] rounded-full opacity-40 blur-[100px]"
        style={{
          background:
            "radial-gradient(circle, rgba(94,234,212,0.25), transparent 65%)",
          animation: "float-slow 11s ease-in-out infinite reverse",
        }}
      />
    </div>
  );
}

function AppPreview() {
  return (
    <div className="glass-strong relative mx-auto max-w-3xl overflow-hidden rounded-3xl p-2 shadow-2xl">
      <div className="rounded-[20px] bg-black/40 p-5">
        <div className="flex gap-4">
          {/* faux sidebar */}
          <div className="hidden w-40 shrink-0 flex-col gap-1.5 sm:flex">
            <div className="mb-2 flex items-center gap-2 px-1">
              <Logo className="h-7 w-7" />
              <div className="text-sm font-semibold">MyVPN</div>
            </div>
            {["Dashboard", "Host", "Connect", "Settings"].map((l, i) => (
              <div
                key={l}
                className={`flex items-center gap-2 rounded-lg px-2.5 py-2 text-xs ${
                  i === 0 ? "glass text-ink" : "text-faint"
                }`}
              >
                <span className="h-1.5 w-1.5 rounded-full bg-current opacity-60" />
                {l}
              </div>
            ))}
          </div>

          {/* main */}
          <div className="flex-1">
            <div className="glass flex flex-col items-center rounded-2xl px-4 py-7">
              <div className="relative grid h-28 w-28 place-items-center">
                {[0, 1].map((i) => (
                  <motion.span
                    key={i}
                    className="absolute rounded-full border border-accent/50"
                    initial={{ width: 64, height: 64, opacity: 0.5 }}
                    animate={{ width: 112, height: 112, opacity: 0 }}
                    transition={{
                      duration: 2.4,
                      repeat: Infinity,
                      delay: i * 1.2,
                      ease: "easeOut",
                    }}
                  />
                ))}
                <div className="ring-1 ring-accent/40 grid h-20 w-20 place-items-center rounded-full bg-[color:var(--color-accent-dim)]">
                  <ShieldCheck className="h-9 w-9 text-accent" strokeWidth={1.5} />
                </div>
              </div>
              <div className="mt-4 text-base font-semibold">Home</div>
              <div className="text-xs text-accent">
                Connected · all traffic routed through host
              </div>
            </div>

            <div className="mt-3 grid grid-cols-3 gap-2.5">
              {[
                { l: "Download", v: "24.1 MB/s" },
                { l: "Latency", v: "12 ms" },
                { l: "Link", v: "Direct P2P" },
              ].map((s) => (
                <div key={s.l} className="glass rounded-xl px-3 py-2.5">
                  <div className="text-[10px] uppercase tracking-wide text-faint">
                    {s.l}
                  </div>
                  <div className="text-sm font-semibold text-ink">{s.v}</div>
                </div>
              ))}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

export function Hero() {
  return (
    <section className="relative overflow-hidden px-4 pt-36 pb-20">
      <div className="grid-fade absolute inset-0 -z-10 h-[600px]" />
      <FloatingOrbs />

      <div className="mx-auto max-w-3xl text-center">
        <motion.div
          initial={{ opacity: 0, y: 12 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.5 }}
          className="glass mx-auto mb-6 inline-flex items-center gap-2 rounded-full px-3.5 py-1.5 text-xs text-muted"
        >
          <Wifi className="h-3.5 w-3.5 text-accent" />
          No server. No subscription. Just yours.
        </motion.div>

        <motion.h1
          initial={{ opacity: 0, y: 16 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.6, delay: 0.05 }}
          className="text-balance text-5xl font-semibold leading-[1.05] tracking-tight sm:text-6xl"
        >
          <span className="text-gradient">Your own</span>{" "}
          <span className="text-gradient-accent">serverless VPN</span>
        </motion.h1>

        <motion.p
          initial={{ opacity: 0, y: 16 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.6, delay: 0.12 }}
          className="mx-auto mt-5 max-w-xl text-pretty text-[15px] leading-relaxed text-muted"
        >
          Host your network on one device and securely route everything from
          another — end-to-end encrypted, peer-to-peer, with{" "}
          <span className="text-ink">no middleware server</span> in between.
        </motion.p>

        <motion.div
          initial={{ opacity: 0, y: 16 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.6, delay: 0.2 }}
          className="mt-8 flex flex-col items-center justify-center gap-3 sm:flex-row"
        >
          <Button asChild size="lg">
            <a href={site.download}>
              <Download className="h-4 w-4" />
              Download for Windows
            </a>
          </Button>
          <Button asChild variant="outline" size="lg">
            <a href={site.github} target="_blank" rel="noreferrer">
              <GithubMark className="h-4 w-4" />
              View on GitHub
            </a>
          </Button>
        </motion.div>

        <p className="mt-4 text-xs text-faint">
          Windows 10 / 11 · lightweight installer · admin required for the tunnel
        </p>
      </div>

      <motion.div
        initial={{ opacity: 0, y: 40, scale: 0.97 }}
        animate={{ opacity: 1, y: 0, scale: 1 }}
        transition={{ duration: 0.8, delay: 0.28, ease: [0.22, 1, 0.36, 1] }}
        className="mt-16"
      >
        <AppPreview />
      </motion.div>

      <div className="mt-12 flex justify-center text-faint">
        <ArrowDown className="h-5 w-5 animate-bounce" />
      </div>
    </section>
  );
}
