import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { toast } from "sonner";
import { open } from "@tauri-apps/plugin-dialog";
import { ArrowDown, ArrowLeft, ArrowRight, ArrowUp, ImageIcon, Info, Loader2, Moon, Sparkles } from "lucide-react";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Label } from "@/components/ui/label";
import { Slider } from "@/components/ui/slider";
import { Switch } from "@/components/ui/switch";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { Keyboard } from "@/components/keyboard";
import { PresetBar } from "@/components/preset-bar";
import { ColourList, ColourPresets, MAX_COLOURS } from "@/components/colour-list";
import { cn } from "@/lib/utils";
import { api, AnimDirection, AnimFrame, AnimKind, AnimSettings, AnimState, Catalog, ImageFit, ImageInfo, Presets, Rgb, toHex } from "@/lib/api";

const KINDS: { id: AnimKind; label: string; help: string; soon?: boolean }[] = [
  { id: "stream", label: "Stream", help: "Your colours flow across the board" },
  { id: "breathing", label: "Breathing", help: "Fades in and out, a new colour each breath" },
  { id: "wave", label: "Wave", help: "A bright crest sweeps over your colours" },
  { id: "sparkle", label: "Sparkle", help: "Keys twinkle in your colours over a dim glow" },
  { id: "gradient", label: "Gradient", help: "Your colours spread across the board, still" },
  // works, but doesn't look good enough yet over 2.4G (about 11 colours a frame)
  { id: "image", label: "Image", help: "Plays a GIF or picture on the keys", soon: true },
];

const FITS: { id: ImageFit; label: string; help: string }[] = [
  { id: "fill", label: "Fill", help: "Covers the board; the edges may be cut off" },
  { id: "fit", label: "Fit", help: "Shows the whole picture; the rest stays dark" },
  { id: "stretch", label: "Stretch", help: "Squeezes the picture to the board's shape" },
];

/** The file name of a path, for showing which picture is chosen. */
const fileName = (path: string) => path.split(/[\\/]/).pop() ?? path;


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
  const [mine, setMine] = useState<Presets["animations"]>([]);
  const [importing, setImporting] = useState(false);
  const [info, setInfo] = useState<ImageInfo | null>(null);
  const pending = useRef<number | null>(null);

  useEffect(() => {
    api.animGet().then(setState);
    api.presetsGet().then((p) => setMine(p.animations)).catch(() => {});
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

  const chooseImage = async () => {
    const path = await open({
      title: "Choose a GIF or picture",
      filters: [{ name: "Images", extensions: ["gif", "png", "jpg", "jpeg", "webp"] }],
    });
    if (typeof path !== "string") return;
    setImporting(true);
    try {
      const i = await api.animImageImport(path);
      setInfo(i);
      set({ kind: "image", image: i.path });
    } catch (e) {
      toast.error("Can't use that picture", { description: String(e) });
    } finally {
      setImporting(false);
    }
  };

  const colours = new Map<number, Rgb>(frame?.keys ?? []);
  const isImage = s.kind === "image";
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
              disabled={k.soon}
              onClick={() => set({ kind: k.id })}
            >
              <span className="flex items-center gap-2">
                {k.label}
                {k.soon && (
                  <Badge variant="outline" className="text-[10px]">
                    Under construction
                  </Badge>
                )}
              </span>
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
                <Sparkles className="size-5" /> {isImage ? "Your picture, on the keys" : "Your colours, animated"}
                {frame?.paused && (
                  <Badge variant="outline" className="gap-1">
                    <Moon className="size-3" /> paused while idle
                  </Badge>
                )}
              </CardTitle>
              <CardDescription>Streamed from this app, so it shows while Lumen75 runs (also from the tray).</CardDescription>
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
          {isImage ? (
            <CardHeader>
              <CardTitle className="text-base">Picture</CardTitle>
              <CardDescription>
                A GIF plays with its own timing; a still picture just shows. Bold, simple images look best: the keyboard
                is only about 16 keys wide.
              </CardDescription>
            </CardHeader>
          ) : (
            <CardHeader>
              <CardTitle className="text-base">Colours</CardTitle>
              <CardDescription>Up to {MAX_COLOURS}. The animation blends from one to the next.</CardDescription>
            </CardHeader>
          )}
          <CardContent className="grid gap-4">
            {isImage ? (
              <>
                <div className="flex flex-wrap items-center gap-3">
                  <Button variant="outline" onClick={chooseImage} disabled={importing}>
                    {importing ? <Loader2 className="animate-spin" /> : <ImageIcon />}
                    {s.image ? "Choose another" : "Choose a GIF or picture"}
                  </Button>
                  {s.image ? (
                    <span className="text-sm">
                      {fileName(s.image)}
                      {info?.path === s.image && (
                        <span className="text-muted-foreground">
                          {" "}
                          · {info.frames === 1 ? "still picture" : `${info.frames} frames, ${info.seconds.toFixed(1)} s`}
                        </span>
                      )}
                    </span>
                  ) : (
                    <span className="text-muted-foreground text-sm">No picture yet.</span>
                  )}
                </div>
                <div className="grid gap-2">
                  <Label>Fit</Label>
                  <ToggleGroup type="single" variant="outline" value={s.fit} onValueChange={(v) => v && set({ fit: v as ImageFit })} className="justify-start">
                    {FITS.map((f) => (
                      <ToggleGroupItem key={f.id} value={f.id} title={f.help}>
                        {f.label}
                      </ToggleGroupItem>
                    ))}
                  </ToggleGroup>
                  <p className="text-muted-foreground text-xs">{FITS.find((f) => f.id === s.fit)?.help}.</p>
                </div>
              </>
            ) : (
              <>
                <ColourList colors={s.colors} onChange={(colors) => set({ colors })} />
                <ColourPresets onPick={(colors) => set({ colors })} />
              </>
            )}
            <PresetBar
              title="My presets"
              saves="this animation: its type, colours or picture, speed, brightness and direction"
              items={mine.map((p) => ({ name: p.name, swatch: p.settings.colors.map(toHex) }))}
              onLoad={(name) => {
                const p = mine.find((x) => x.name === name);
                if (p) set(p.settings);
              }}
              onSave={async (name) => {
                try {
                  setMine((await api.animPresetSave(name, s)).animations);
                  toast.success(`Saved “${name.trim()}”`);
                  return true;
                } catch (e) {
                  toast.error("Can't save the preset", { description: String(e) });
                  return false;
                }
              }}
              onDelete={async (name) => {
                setMine((await api.presetDelete("animation", name)).animations);
              }}
            />
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
          streams about 14 frames a second (about 11 over Bluetooth), pauses when the computer is idle (see Device → App), and the keyboard shows
          its own effect again about 2 seconds after the app stops. Over 2.4G and Bluetooth a frame holds about 11
          colours, so pictures show with fewer shades; over the USB cable every key gets its own colour.
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
