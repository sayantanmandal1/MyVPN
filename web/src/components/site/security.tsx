import {
  Lock,
  Fingerprint,
  ShieldX,
  EyeOff,
  KeyRound,
  Server,
  ShieldCheck,
} from "lucide-react";
import { Reveal } from "@/components/ui/reveal";

const points = [
  {
    Icon: Lock,
    title: "TLS 1.3 everywhere",
    desc: "Every byte between your devices is encrypted with modern, forward-secret cryptography.",
  },
  {
    Icon: Fingerprint,
    title: "Authenticated peers",
    desc: "Connections are verified by public key. Only devices you approve can join your network.",
  },
  {
    Icon: ShieldX,
    title: "Built-in kill-switch",
    desc: "If the tunnel drops, traffic is blocked instead of leaking onto the open internet.",
  },
  {
    Icon: EyeOff,
    title: "No logs, no tracking",
    desc: "There's no server to collect your activity — because there's no server at all.",
  },
  {
    Icon: KeyRound,
    title: "Local private identity",
    desc: "Your device key is generated on-device and stored encrypted at rest with Windows DPAPI, tied to your user account.",
  },
  {
    Icon: Server,
    title: "No central server",
    desc: "Discovery uses public infrastructure you don't run, and connections go direct peer-to-peer whenever possible — end-to-end encryption means nothing in the middle can read your traffic.",
  },
];

export function Security() {
  return (
    <section id="security" className="px-4 py-24">
      <div className="mx-auto max-w-5xl">
        <div className="glass-strong overflow-hidden rounded-3xl p-8 sm:p-12">
          <div className="grid gap-10 lg:grid-cols-2 lg:items-center">
            <Reveal>
              <div className="mb-3 text-sm font-medium text-accent">Security</div>
              <h2 className="text-balance text-3xl font-semibold tracking-tight sm:text-4xl">
                Private by design, secure by default
              </h2>
              <p className="mt-4 max-w-md text-pretty text-[15px] leading-relaxed text-muted">
                MyVPN treats your traffic as nobody&apos;s business but yours.
                Encryption and authentication aren&apos;t add-ons — they&apos;re
                the foundation everything else is built on.
              </p>
              <div className="glass mt-6 inline-flex items-center gap-2 rounded-full px-3 py-1.5 text-xs text-muted">
                <ShieldCheck className="h-3.5 w-3.5 text-accent" />
                Hardened against the OWASP Top 10
              </div>
            </Reveal>

            <div className="grid gap-3 sm:grid-cols-2">
              {points.map((p, i) => (
                <Reveal key={p.title} delay={(i % 2) * 0.08}>
                  <div className="glass h-full rounded-2xl p-4">
                    <p.Icon className="h-5 w-5 text-accent" />
                    <div className="mt-2.5 text-sm font-semibold">{p.title}</div>
                    <div className="mt-1 text-xs leading-relaxed text-muted">
                      {p.desc}
                    </div>
                  </div>
                </Reveal>
              ))}
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}
