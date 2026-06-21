import { ArrowDown, ArrowUp } from "lucide-react";
import { formatRate } from "../lib/format";
import { cn } from "../lib/utils";

function Spark({ data, color }: { data: number[]; color: string }) {
  const max = Math.max(1, ...data);
  return (
    <div className="flex h-12 items-end gap-[3px]">
      {data.map((v, i) => (
        <div
          key={i}
          className="w-full rounded-sm transition-all duration-300"
          style={{
            height: `${Math.max(4, (v / max) * 100)}%`,
            background: color,
            opacity: 0.35 + (i / data.length) * 0.65,
          }}
        />
      ))}
    </div>
  );
}

export function ThroughputMeter({
  rateUp,
  rateDown,
  historyUp,
  historyDown,
}: {
  rateUp: number;
  rateDown: number;
  historyUp: number[];
  historyDown: number[];
}) {
  const rows = [
    {
      label: "Download",
      rate: rateDown,
      data: historyDown,
      color: "var(--color-accent)",
      Icon: ArrowDown,
    },
    {
      label: "Upload",
      rate: rateUp,
      data: historyUp,
      color: "#7dd3fc",
      Icon: ArrowUp,
    },
  ];

  return (
    <div className="grid grid-cols-2 gap-3">
      {rows.map(({ label, rate, data, color, Icon }) => (
        <div key={label} className="glass rounded-2xl p-4">
          <div className="mb-3 flex items-center justify-between">
            <div className="flex items-center gap-2">
              <Icon className="h-4 w-4" style={{ color }} />
              <span className="text-xs font-medium text-[color:var(--color-muted)]">
                {label}
              </span>
            </div>
            <span
              className={cn("text-sm font-semibold tabular-nums")}
              style={{ color }}
            >
              {formatRate(rate)}
            </span>
          </div>
          <Spark data={data} color={color} />
        </div>
      ))}
    </div>
  );
}
