import { cn } from "@/lib/utils";
import type { Key } from "@/lib/api";

export interface KeyLook {
  /** Keycap fill colour (CSS). */
  fill?: string;
  /** Small second line, e.g. what a key is remapped to. */
  sub?: string;
  tone?: "changed" | "off";
}

interface Props {
  keys: Key[];
  selected: Set<number>;
  onKey?: (index: number, additive: boolean) => void;
  look?: (index: number) => KeyLook;
  /** Keys that can't be selected on this page. */
  disabled?: (index: number) => boolean;
}

const VIEW = { x: 22, y: 22, w: 616, h: 247 };
const KNOB = { cx: 617, cy: 43, r: 12 };

/** The F75 drawn from the official app's key positions. Click selects; Ctrl/Shift-click adds. */
export function Keyboard({ keys, selected, onKey, look, disabled }: Props) {
  const knob = keys.find((k) => k.name === "knob");
  return (
    <svg viewBox={`${VIEW.x} ${VIEW.y} ${VIEW.w} ${VIEW.h}`} className="w-full select-none" role="group" aria-label="Keyboard">
      <rect x={VIEW.x} y={VIEW.y} width={VIEW.w} height={VIEW.h} rx={12} className="fill-neutral-950 stroke-neutral-800" />
      {keys
        .filter((k) => k.rect[2] > 0)
        .map((k) => {
          const [x1, y1, x2, y2] = k.rect;
          const l = look?.(k.index) ?? {};
          const sel = selected.has(k.index);
          const off = disabled?.(k.index);
          const w = x2 - x1;
          const h = y2 - y1;
          return (
            <g
              key={k.index}
              role="button"
              aria-pressed={sel}
              aria-label={k.label}
              className={cn("group", off ? "cursor-not-allowed opacity-40" : "cursor-pointer")}
              onClick={(e) => !off && onKey?.(k.index, e.ctrlKey || e.shiftKey || e.metaKey)}
            >
              <rect
                x={x1}
                y={y1}
                width={w}
                height={h}
                rx={5}
                style={l.fill ? { fill: l.fill } : undefined}
                className={cn(
                  "stroke-[1.2] transition-colors",
                  !l.fill && "fill-neutral-800",
                  l.tone === "changed" && !l.fill && "fill-amber-900/70",
                  l.tone === "off" && !l.fill && "fill-red-950",
                  sel ? "stroke-white stroke-2" : "stroke-neutral-700 group-hover:stroke-neutral-400",
                )}
              />
              <text
                x={x1 + w / 2}
                y={y1 + (l.sub ? h / 2 - 3 : h / 2 + 1)}
                textAnchor="middle"
                dominantBaseline="middle"
                className={cn("pointer-events-none text-[8.5px]", l.tone === "off" ? "fill-red-300" : "fill-neutral-100")}
                style={l.fill ? { paintOrder: "stroke", stroke: "rgba(0,0,0,.55)", strokeWidth: 2 } : undefined}
              >
                {k.label}
              </text>
              {l.sub && (
                <text
                  x={x1 + w / 2}
                  y={y1 + h / 2 + 8}
                  textAnchor="middle"
                  dominantBaseline="middle"
                  className="pointer-events-none fill-amber-200 text-[6px]"
                >
                  {l.sub}
                </text>
              )}
            </g>
          );
        })}
      {knob && (
        <g
          role="button"
          aria-label="Knob"
          className={cn("group", disabled?.(knob.index) ? "cursor-not-allowed opacity-40" : "cursor-pointer")}
          onClick={(e) => !disabled?.(knob.index) && onKey?.(knob.index, e.ctrlKey || e.shiftKey || e.metaKey)}
        >
          <circle
            {...KNOB}
            style={look?.(knob.index).fill ? { fill: look(knob.index).fill } : undefined}
            className={cn(
              "fill-neutral-700",
              selected.has(knob.index) ? "stroke-white stroke-2" : "stroke-neutral-500 group-hover:stroke-neutral-300",
            )}
          />
          <circle cx={KNOB.cx} cy={KNOB.cy - 7} r={1.6} className="pointer-events-none fill-neutral-300" />
        </g>
      )}
    </svg>
  );
}
