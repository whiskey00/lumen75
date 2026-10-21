import type { Key, Rgb } from "@/lib/api";
import { toHex } from "@/lib/api";

const VIEW = { x: 30, y: 30, w: 600, h: 232 };

/** A small keyboard with no labels: just the keys' colours (unlit keys dark). For previews. */
export function MiniKeyboard({ keys, colours, className }: { keys: Key[]; colours: Map<number, Rgb> | Rgb[]; className?: string }) {
  const at = (i: number) => (Array.isArray(colours) ? colours[i] : colours.get(i));
  return (
    <svg viewBox={`${VIEW.x} ${VIEW.y} ${VIEW.w} ${VIEW.h}`} className={className} aria-hidden>
      {keys
        .filter((k) => k.rect[2] > 0)
        .map((k) => {
          const [x1, y1, x2, y2] = k.rect;
          const c = at(k.index);
          const lit = c && c.some((v) => v > 0);
          return <rect key={k.index} x={x1} y={y1} width={x2 - x1} height={y2 - y1} rx={4} fill={lit ? toHex(c) : "#262626"} />;
        })}
    </svg>
  );
}
