import { Plus, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { ColourInput } from "@/components/colour-input";
import { fromHex, Rgb, toHex } from "@/lib/api";

export const MAX_COLOURS = 7;

/** Edit a list of 1-7 colours: change one, add one (copies the last), remove one. */
export function ColourList({ colors, onChange }: { colors: Rgb[]; onChange: (c: Rgb[]) => void }) {
  return (
    <div className="flex flex-wrap items-center gap-2">
      {colors.map((c, i) => (
        <div key={i} className="group relative">
          <ColourInput
            stacked
            value={toHex(c)}
            onChange={(hex) => onChange(colors.map((x, j) => (j === i ? fromHex(hex) : x)))}
            label={`Colour ${i + 1}`}
          />
          {colors.length > 1 && (
            <button
              aria-label={`Remove colour ${i + 1}`}
              onClick={() => onChange(colors.filter((_, j) => j !== i))}
              className="bg-background absolute -top-2 -right-2 hidden rounded-full border p-0.5 group-hover:block"
            >
              <X className="size-3" />
            </button>
          )}
        </div>
      ))}
      {colors.length < MAX_COLOURS && (
        <Button
          variant="outline"
          size="icon"
          className="h-11 w-20"
          aria-label="Add a colour"
          onClick={() => onChange([...colors, colors[colors.length - 1] ?? [255, 255, 255]])}
        >
          <Plus />
        </Button>
      )}
    </div>
  );
}

/** Ready-made colour sets, shared by the animations and the audio lighting. */
export const COLOUR_PRESETS: { name: string; colors: string[] }[] = [
  { name: "Sunset", colors: ["#ff1493", "#ff0000", "#ff6e00"] },
  { name: "Ocean", colors: ["#00ffd5", "#0080ff", "#2000ff"] },
  { name: "Forest", colors: ["#00ff40", "#a0ff00", "#008a20"] },
  { name: "Synthwave", colors: ["#ff00c8", "#8a00ff", "#00e5ff"] },
  { name: "Fire", colors: ["#ff0000", "#ff5000", "#ffb000"] },
  { name: "Ice", colors: ["#ffffff", "#9ad8ff", "#2a7bff"] },
];

export function ColourPresets({ onPick }: { onPick: (c: Rgb[]) => void }) {
  return (
    <div className="flex flex-wrap items-center gap-2">
      <span className="text-muted-foreground text-sm">Colour sets:</span>
      {COLOUR_PRESETS.map((p) => (
        <Button key={p.name} size="sm" variant="outline" className="gap-2" onClick={() => onPick(p.colors.map(fromHex))}>
          <span className="flex">
            {p.colors.map((c) => (
              <span key={c} className="-ml-1 size-3 rounded-full border border-black/40 first:ml-0" style={{ background: c }} />
            ))}
          </span>
          {p.name}
        </Button>
      ))}
    </div>
  );
}
