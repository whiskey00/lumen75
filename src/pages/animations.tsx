import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { toast } from "sonner";
import { ArrowDown, ArrowLeft, ArrowRight, ArrowUp, Info, Moon, Plus, Sparkles, X } from "lucide-react";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Label } from "@/components/ui/label";
import { Slider } from "@/components/ui/slider";
import { Switch } from "@/components/ui/switch";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { Keyboard } from "@/components/keyboard";
import { cn } from "@/lib/utils";
import { api, AnimDirection, AnimFrame, AnimKind, AnimSettings, AnimState, Catalog, fromHex, Rgb, toHex } from "@/lib/api";

const KINDS: { id: AnimKind; label: string; help: string }[] = [
  { id: "stream", label: "Stream", help: "Your colours flow across the board" },
  { id: "breathing", label: "Breathing", help: "Fades in and out, a new colour each breath" },
  { id: "wave", label: "Wave", help: "A bright crest sweeps over your colours" },
  { id: "sparkle", label: "Sparkle", help: "Keys twinkle in your colours over a dim glow" },
  { id: "gradient", label: "Gradient", help: "Your colours spread across the board, still" },
];

const PRESETS: { name: string; colors: string[] }[] = [
  { name: "Sunset", colors: ["#ff1493", "#ff0000", "#ff6e00"] },
  { name: "Ocean", colors: ["#00ffd5", "#0080ff", "#2000ff"] },
  { name: "Forest", colors: ["#00ff40", "#a0ff00", "#008a20"] },
  { name: "Synthwave", colors: ["#ff00c8", "#8a00ff", "#00e5ff"] },
  { name: "Fire", colors: ["#ff0000", "#ff5000", "#ffb000"] },
  { name: "Ice", colors: ["#ffffff", "#9ad8ff", "#2a7bff"] },
];

const MAX_COLORS = 7;
const DIRECTIONS: { id: AnimDirection; icon: typeof ArrowRight }[] = [
  { id: "right", icon: ArrowRight },
  { id: "left", icon: ArrowLeft },
  { id: "up", icon: ArrowUp },
  { id: "down", icon: ArrowDown },
];

export function AnimationsPage({ catalog }: { catalog: Catalog }) {
  const [state, setState] = useState<AnimState | null>(null);
  const [frame, setFrame] = useState<AnimFrame | null>(null);
  const [busy, setBusy] = useState(false);
  const pending = useRef<number | null>(null);

  useEffect(() => {
    api.animGet().then(setState);
    const offs = [
      listen<AnimFrame>("anim-frame", (e) => setFrame(e.payload)),
      listen<AnimState>("anim-state", (e) => {
        setState(e.payload);
        if (!e.payload.running) setFrame(null);
      }),
      listen<string>("anim-error", (e) => toast.error("Animation", { description: e.payload })),
    ];
    return () => offs.forEach((p) => p.then((off) => off()));
  }, []);

  if (!state) return null;
  const s = state.settings;

  /** Change settings: shown at once, saved and sent shortly after (sliders send many). */
  const set = (p: Partial<AnimSettings>) => {
    const next = { ...s, ...p };
    setState({ ...state, settings: next });
    if (pending.current) window.clearTimeout(pending.current);
    pending.current = window.setTimeout(() => api.animUpdate(next).catch(() => {}), 120);
  };

  const toggle = async (on: boolean) => {
    setBusy(true);
    try {
      setState(on ? await api.animStart(s) : await api.animStop());
      if (!on) setFrame(null);
    } catch (e) {
      toast.error(on ? "Can't start the animation" : "Can't stop the animation", { description: String(e) });
    } finally {
      setBusy(false);
    }
  };

  const colours = new Map<number, Rgb>(frame?.keys ?? []);
  const usesDirection = s.kind === "stream" || s.kind === "wave" || s.kind === "gradient";

  return (
    <div className="grid gap-4 lg:grid-cols-[220px_1fr]">
      <Card className="self-start">
        <CardHeader>
          <CardTitle>Animation</CardTitle>
        </CardHeader>
        <CardContent className="grid gap-1">
          {KINDS.map((k) => (
            <Button
              key={k.id}
              variant={s.kind === k.id ? "secondary" : "ghost"}
              className="h-auto flex-col items-start gap-0.5 py-2 text-left whitespace-normal"
              onClick={() => set({ kind: k.id })}
            >
              <span>{k.label}</span>
              <span className="text-muted-foreground text-xs font-normal">{k.help}</span>
            </Button>
          ))}
        </CardContent>
      </Card>

      <div className="grid gap-4">
        <Card>
          <CardHeader className="flex flex-row items-start justify-between gap-4">
            <div className="grid gap-1.5">
              <CardTitle className="flex items-center gap-2">
                <Sparkles className="size-5" /> Your colours, animated
                {frame?.paused && (
                  <Badge variant="outline" className="gap-1">
                    <Moon className="size-3" /> paused while idle
                  </Badge>
                )}
              </CardTitle>
              <CardDescription>Streamed from this app, so it shows while AULA Studio runs (also from the tray).</CardDescription>
            </div>
            <div className="flex items-center gap-2 text-sm">
              <span className={cn(!state.running && "text-foreground font-medium")}>Off</span>
              <Switch checked={state.running} disabled={busy} onCheckedChange={toggle} aria-label="Animation" />
              <span className={cn(state.running && "text-foreground font-medium")}>On</span>
            </div>
          </CardHeader>
          <CardContent>
            <Keyboard
              keys={catalog.keys}
              selected={new Set()}
              look={(i) => {
                const c = colours.get(i);
                return c ? { fill: toHex(c) } : {};
              }}
            />
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle className="text-base">Colours</CardTitle>
            <CardDescription>Up to {MAX_COLORS}. The animation blends from one to the next.</CardDescription>
          </CardHeader>
          <CardContent className="grid gap-4">
            <div className="flex flex-wrap items-center gap-2">
              {s.colors.map((c, i) => (
                <div key={i} className="group relative">
                  <input
                    type="color"
                    value={toHex(c)}
                    onChange={(e) => set({ colors: s.colors.map((x, j) => (j === i ? fromHex(e.target.value) : x)) })}
                    className="h-11 w-14 cursor-pointer rounded-md border bg-transparent"
                    aria-label={`Colour ${i + 1}`}
                  />
                  {s.colors.length > 1 && (
                    <button
                      aria-label={`Remove colour ${i + 1}`}
                      onClick={() => set({ colors: s.colors.filter((_, j) => j !== i) })}
                      className="bg-background absolute -top-2 -right-2 hidden rounded-full border p-0.5 group-hover:block"
                    >
                      <X className="size-3" />
                    </button>
                  )}
                </div>
              ))}
              {s.colors.length < MAX_COLORS && (
                <Button
                  variant="outline"
                  size="icon"
                  className="h-11 w-14"
                  aria-label="Add a colour"
                  onClick={() => set({ colors: [...s.colors, s.colors[s.colors.length - 1] ?? [255, 255, 255]] })}
                >
                  <Plus />
                </Button>
              )}
            </div>
            <div className="flex flex-wrap items-center gap-2">
              <span className="text-muted-foreground text-sm">Presets:</span>
              {PRESETS.map((p) => (
                <Button key={p.name} size="sm" variant="outline" className="gap-2" onClick={() => set({ colors: p.colors.map(fromHex) })}>
                  <span className="flex">
                    {p.colors.map((c) => (
                      <span key={c} className="-ml-1 size-3 rounded-full border border-black/40 first:ml-0" style={{ background: c }} />
                    ))}
                  </span>
                  {p.name}
                </Button>
              ))}
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardContent className="grid gap-6 pt-6 md:grid-cols-2">
            <Level label="Speed" value={s.speed} min={0.1} max={3} step={0.1} text={`${s.speed.toFixed(1)}×`} onChange={(v) => set({ speed: v })} />
            <Level
              label="Brightness"
              value={s.brightness}
              min={0.05}
              max={1}
              step={0.05}
              text={`${Math.round(s.brightness * 100)}%`}
              onChange={(v) => set({ brightness: v })}
            />
            <div className={cn("grid gap-3", !usesDirection && "pointer-events-none opacity-40")}>
              <Label>Direction</Label>
              <ToggleGroup type="single" variant="outline" value={s.direction} onValueChange={(v) => v && set({ direction: v as AnimDirection })} className="justify-start">
                {DIRECTIONS.map((d) => (
                  <ToggleGroupItem key={d.id} value={d.id} aria-label={d.id}>
                    <d.icon />
                  </ToggleGroupItem>
                ))}
              </ToggleGroup>
            </div>
          </CardContent>
        </Card>

        <p className="text-muted-foreground flex items-start gap-2 text-xs">
          <Info className="mt-0.5 size-3.5 shrink-0" />
          The keyboard can't store animations like these: its own multicolour effects use a fixed rainbow. So this
          streams about 14 frames a second, pauses when the computer is idle (see Device → App), and the keyboard shows
          its own effect again about 2 seconds after the app stops.
        </p>
      </div>
    </div>
  );
}

function Level(props: { label: string; value: number; min: number; max: number; step: number; text: string; onChange: (v: number) => void }) {
  return (
    <div className="grid gap-3">
      <div className="flex items-center justify-between text-sm">
        <span className="font-medium">{props.label}</span>
        <span className="text-muted-foreground tabular-nums">{props.text}</span>
      </div>
      <Slider min={props.min} max={props.max} step={props.step} value={[props.value]} onValueChange={([v]) => props.onChange(v)} />
    </div>
  );
}
