import { useEffect, useState } from "react";
import { ArrowLeft, ArrowRight, Copy, Eraser, PaintBucket, Paintbrush, Plus, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { Slider } from "@/components/ui/slider";
import { ColourInput } from "@/components/colour-input";
import { Keyboard } from "@/components/keyboard";
import { MiniKeyboard } from "@/components/mini-keyboard";
import { cn } from "@/lib/utils";
import { Catalog, CustomFrame, fromHex, Rgb, toHex } from "@/lib/api";

const SWATCHES = ["#ff0000", "#ff6000", "#ffd000", "#00ff40", "#00e5ff", "#0040ff", "#8a00ff", "#ff00b0", "#ffffff"];
/** Frames are stored by matrix index; the F75's indexes go up to 125. */
const SLOTS = 126;
const MAX_FRAMES = 60;

export const blankFrame = (fill: Rgb = [0, 0, 0]): CustomFrame => ({ colors: Array.from({ length: SLOTS }, () => fill), hold_ms: 500, fade_ms: 300 });

/**
 * Paint your own animation: a strip of frames, each with every key's colour, how long it
 * shows and how long it fades into the next. The page's preview plays it as you go.
 */
export function FrameEditor({ catalog, frames, onChange }: { catalog: Catalog; frames: CustomFrame[]; onChange: (f: CustomFrame[]) => void }) {
  const [cur, setCur] = useState(0);
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [colour, setColour] = useState("#ff0000");
  const keys = catalog.keys.filter((k) => k.rect[2] > 0);
  const at = Math.min(cur, frames.length - 1);
  const frame = frames[at];

  useEffect(() => {
    if (cur > frames.length - 1) setCur(Math.max(0, frames.length - 1));
  }, [frames.length, cur]);

  if (!frame) return null;

  const update = (i: number, f: Partial<CustomFrame>) => onChange(frames.map((x, j) => (j === i ? { ...x, ...f } : x)));
  const paint = (rgb: Rgb, which: Iterable<number>) => {
    const colors = frame.colors.slice();
    for (const i of which) colors[i] = rgb;
    update(at, { colors });
  };
  const insert = (f: CustomFrame) => {
    if (frames.length >= MAX_FRAMES) return;
    onChange([...frames.slice(0, at + 1), f, ...frames.slice(at + 1)]);
    setCur(at + 1);
  };
  const move = (by: number) => {
    const to = at + by;
    if (to < 0 || to >= frames.length) return;
    const next = frames.slice();
    [next[at], next[to]] = [next[to], next[at]];
    onChange(next);
    setCur(to);
  };
  const total = frames.reduce((s, f) => s + f.hold_ms + f.fade_ms, 0) / 1000;

  return (
    <div className="grid gap-4">
      {/* the frames */}
      <div className="flex items-stretch gap-2 overflow-x-auto pb-1">
        {frames.map((f, i) => (
          <button
            key={i}
            type="button"
            onClick={() => setCur(i)}
            className={cn(
              "bg-card hover:border-primary/50 grid w-28 shrink-0 gap-1 rounded-md border p-1.5 text-left transition-colors",
              i === at && "border-primary ring-primary/30 ring-2",
            )}
          >
            <MiniKeyboard keys={catalog.keys} colours={f.colors} className="w-full" />
            <span className="text-muted-foreground text-[11px]">
              {i + 1} · {(f.hold_ms / 1000).toFixed(1)}s{f.fade_ms > 0 && ` + fade ${(f.fade_ms / 1000).toFixed(1)}s`}
            </span>
          </button>
        ))}
        <div className="grid shrink-0 content-start gap-1">
          <Button size="sm" variant="outline" disabled={frames.length >= MAX_FRAMES} onClick={() => insert({ ...frame, colors: frame.colors.slice() })} title="Copy this frame after it">
            <Copy /> Copy
          </Button>
          <Button size="sm" variant="outline" disabled={frames.length >= MAX_FRAMES} onClick={() => insert(blankFrame())} title="A dark frame after this one">
            <Plus /> Blank
          </Button>
        </div>
      </div>

      <div className="flex flex-wrap items-center gap-2 text-sm">
        <span className="font-medium">Frame {at + 1} of {frames.length}</span>
        <span className="text-muted-foreground">· the loop lasts {total.toFixed(1)} s at speed 1×</span>
        <div className="ml-auto flex gap-1">
          <Button size="sm" variant="ghost" disabled={at === 0} onClick={() => move(-1)} title="Move earlier">
            <ArrowLeft />
          </Button>
          <Button size="sm" variant="ghost" disabled={at === frames.length - 1} onClick={() => move(1)} title="Move later">
            <ArrowRight />
          </Button>
          <Button
            size="sm"
            variant="ghost"
            disabled={frames.length <= 1}
            onClick={() => onChange(frames.filter((_, j) => j !== at))}
            title="Delete this frame"
          >
            <Trash2 />
          </Button>
        </div>
      </div>

      {/* painting the frame */}
      <Keyboard
        keys={catalog.keys}
        selected={selected}
        onSelect={setSelected}
        look={(i) => {
          const c = frame.colors[i];
          return c && c.some((v) => v) ? { fill: toHex(c) } : {};
        }}
      />
      <div className="flex flex-wrap items-center gap-3">
        <ColourInput value={colour} onChange={setColour} label="Colour" />
        <div className="flex gap-1.5">
          {SWATCHES.map((s) => (
            <button
              key={s}
              aria-label={s}
              onClick={() => setColour(s)}
              className={cn("size-6 rounded-full border-2 transition-transform hover:scale-110", s === colour ? "border-white" : "border-transparent")}
              style={{ background: s }}
            />
          ))}
        </div>
        <div className="ml-auto flex flex-wrap gap-2">
          <Button size="sm" variant="secondary" disabled={selected.size === 0} onClick={() => paint(fromHex(colour), selected)}>
            <Paintbrush /> Paint selected
          </Button>
          <Button size="sm" variant="outline" disabled={selected.size === 0} onClick={() => paint([0, 0, 0], selected)}>
            <Eraser /> Turn off
          </Button>
          <Button size="sm" variant="outline" onClick={() => paint(fromHex(colour), keys.map((k) => k.index))}>
            <PaintBucket /> Fill frame
          </Button>
        </div>
      </div>
      <p className="text-muted-foreground text-xs">Drag over keys to select several, Ctrl/Shift to add more. Then paint them.</p>

      <div className="grid gap-6 md:grid-cols-2">
        <Timing label="Shows for" ms={frame.hold_ms} min={50} max={5000} onChange={(hold_ms) => update(at, { hold_ms })} />
        <Timing label="Fades into the next for" ms={frame.fade_ms} min={0} max={3000} onChange={(fade_ms) => update(at, { fade_ms })} />
      </div>
    </div>
  );
}

function Timing({ label, ms, min, max, onChange }: { label: string; ms: number; min: number; max: number; onChange: (ms: number) => void }) {
  return (
    <div className="grid gap-3">
      <div className="flex items-center justify-between text-sm">
        <Label>{label}</Label>
        <span className="text-muted-foreground tabular-nums">{ms === 0 ? "no fade" : `${(ms / 1000).toFixed(2)} s`}</span>
      </div>
      <Slider min={min} max={max} step={50} value={[ms]} onValueChange={([v]) => onChange(v)} />
    </div>
  );
}
