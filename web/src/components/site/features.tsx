import {
  RadioTower,
  Globe2,
  Network,
  ShieldCheck,
  Power,
  Waypoints,
} from "lucide-react";
import { Reveal } from "@/components/ui/reveal";

const features = [
  {
    Icon: RadioTower,
    title: "Host in one click",
    desc: "Turn any Windows device into a VPN. Name your network and you're live — no config files, no certificates to wrangle.",
  },
  {
    Icon: Globe2,
    title: "Connect from anywhere",
    desc: "On the same Wi‑Fi, hosts appear automatically. Across the internet, the host's pairing code — with an optional passphrase — is all it takes.",
  },
  {
    Icon: Network,
    title: "True full tunnel",
    desc: "Route every packet through your host — browse with its IP and reach every device on its local network, just like a commercial VPN.",
  },
  {
    Icon: ShieldCheck,
    title: "End-to-end encrypted",
    desc: "Every connection is secured with TLS 1.3 and authenticated by public key. Only devices you approve can ever connect.",
  },
  {
    Icon: Waypoints,
    title: "Truly peer-to-peer",
    desc: "Built on iroh QUIC with NAT hole-punching. Once connected, traffic flows directly between your devices — no server in the middle.",
  },
  {
    Icon: Power,
    title: "Near-zero downtime",
    desc: "Enable “start on boot” and MyVPN re-hosts your network automatically after every restart, running quietly in the system tray.",
  },
];

export function Features() {
  return (
    <section id="features" className="px-4 py-24">
      <div className="mx-auto max-w-5xl">
        <Reveal className="mx-auto max-w-2xl text-center">
          <div className="mb-3 text-sm font-medium text-accent">Features</div>
          <h2 className="text-balance text-3xl font-semibold tracking-tight sm:text-4xl">
            Everything a VPN should be —
            <br className="hidden sm:block" /> and nothing it shouldn&apos;t
          </h2>
          <p className="mx-auto mt-4 max-w-lg text-pretty text-[15px] leading-relaxed text-muted">
            A real VPN you fully own, with the polish of a commercial product and
            none of the servers, subscriptions, or tracking.
          </p>
        </Reveal>

        <div className="mt-14 grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {features.map((f, i) => (
            <Reveal key={f.title} delay={(i % 3) * 0.08}>
              <div className="glass h-full rounded-2xl p-6 transition duration-300 hover:-translate-y-1 hover:bg-white/[0.07]">
                <div className="mb-4 grid h-11 w-11 place-items-center rounded-xl bg-[color:var(--color-accent-dim)] ring-1 ring-accent/20">
                  <f.Icon className="h-5 w-5 text-accent" />
                </div>
                <h3 className="text-base font-semibold">{f.title}</h3>
                <p className="mt-1.5 text-sm leading-relaxed text-muted">
                  {f.desc}
                </p>
              </div>
            </Reveal>
          ))}
        </div>
      </div>
    </section>
  );
}
