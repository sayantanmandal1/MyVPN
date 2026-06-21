import { LayoutDashboard, RadioTower, PlugZap, Globe2, Settings } from "lucide-react";
import { cn } from "../lib/utils";
import type { ConnectionState, VpnRole } from "../lib/api";

export type Route = "dashboard" | "host" | "connect" | "public" | "settings";

type NavItem = { id: Route; label: string; Icon: typeof LayoutDashboard };

const groups: { label: string; items: NavItem[] }[] = [
  {
    label: "Private VPN",
    items: [
      { id: "dashboard", label: "Dashboard", Icon: LayoutDashboard },
      { id: "host", label: "Host", Icon: RadioTower },
      { id: "connect", label: "Connect", Icon: PlugZap },
    ],
  },
  {
    label: "Public VPN",
    items: [{ id: "public", label: "Free servers", Icon: Globe2 }],
  },
];

const bottomItems: NavItem[] = [
  { id: "settings", label: "Settings", Icon: Settings },
];

function stateColor(state: ConnectionState): string {
  switch (state) {
    case "connected":
    case "hosting":
      return "bg-accent";
    case "connecting":
    case "discovering":
    case "reconnecting":
      return "bg-[color:var(--color-warn)]";
    case "error":
      return "bg-[color:var(--color-danger)]";
    default:
      return "bg-white/25";
  }
}

function stateLabel(state: ConnectionState, role: VpnRole): string {
  if (state === "connected") return "Connected";
  if (state === "hosting") return role === "host" ? "Hosting" : "Online";
  if (state === "connecting") return "Connecting…";
  if (state === "discovering") return "Searching…";
  if (state === "reconnecting") return "Reconnecting…";
  if (state === "error") return "Error";
  return "Idle";
}

function NavButton({
  item,
  active,
  onNavigate,
}: {
  item: NavItem;
  active: boolean;
  onNavigate: (r: Route) => void;
}) {
  const { id, label, Icon } = item;
  return (
    <button
      onClick={() => onNavigate(id)}
      className={cn(
        "group flex items-center gap-3 rounded-xl px-3 py-2.5 text-sm transition-all",
        active
          ? "glass text-ink"
          : "text-[color:var(--color-muted)] hover:bg-white/5 hover:text-ink",
      )}
    >
      <Icon
        className={cn(
          "h-[18px] w-[18px] transition-colors",
          active && "text-accent",
        )}
      />
      {label}
      {active && (
        <span className="ml-auto h-1.5 w-1.5 rounded-full bg-accent" />
      )}
    </button>
  );
}

export function Sidebar({
  route,
  onNavigate,
  state,
  role,
}: {
  route: Route;
  onNavigate: (r: Route) => void;
  state: ConnectionState;
  role: VpnRole;
}) {
  return (
    <aside className="glass-strong flex w-60 shrink-0 flex-col rounded-3xl p-3">
      <div className="flex items-center gap-3 px-3 py-4">
        <div className="ring-accent grid h-10 w-10 place-items-center rounded-xl bg-[color:var(--color-accent-dim)]">
          <svg viewBox="0 0 24 24" className="h-5 w-5 text-accent" fill="none">
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
        </div>
        <div>
          <div className="text-[15px] font-semibold tracking-tight">MyVPN</div>
          <div className="text-[11px] text-[color:var(--color-faint)]">
            Serverless VPN
          </div>
        </div>
      </div>

      <nav className="mt-2 flex flex-1 flex-col gap-4">
        {groups.map((group) => (
          <div key={group.label} className="flex flex-col gap-1">
            <div className="px-3 pb-0.5 text-[10px] font-semibold tracking-wider text-[color:var(--color-faint)] uppercase">
              {group.label}
            </div>
            {group.items.map((item) => (
              <NavButton
                key={item.id}
                item={item}
                active={route === item.id}
                onNavigate={onNavigate}
              />
            ))}
          </div>
        ))}
      </nav>

      <div className="flex flex-col gap-1 border-t border-white/5 pt-3">
        {bottomItems.map((item) => (
          <NavButton
            key={item.id}
            item={item}
            active={route === item.id}
            onNavigate={onNavigate}
          />
        ))}
      </div>

      <div className="mt-3">
        <div className="glass flex items-center gap-2.5 rounded-xl px-3 py-2.5">
          <span className="relative flex h-2.5 w-2.5">
            <span
              className={cn(
                "absolute inline-flex h-full w-full rounded-full opacity-60",
                stateColor(state),
                (state === "connected" || state === "hosting") &&
                  "animate-ping",
              )}
            />
            <span
              className={cn(
                "relative inline-flex h-2.5 w-2.5 rounded-full",
                stateColor(state),
              )}
            />
          </span>
          <span className="text-xs font-medium text-[color:var(--color-muted)]">
            {stateLabel(state, role)}
          </span>
        </div>
      </div>
    </aside>
  );
}
