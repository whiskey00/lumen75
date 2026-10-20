import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { toast } from "sonner";
import { AudioLines, Info, VolumeX } from "lucide-react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { Slider } from "@/components/ui/slider";
import { Switch } from "@/components/ui/switch";
import { Keyboard } from "@/components/keyboard";
import { cn } from "@/lib/utils";
import { api, AudioFrame, AudioMode, AudioSettings, Catalog, fromHex, Rgb, toHex } from "@/lib/api";

const MODES: { id: AudioMode; label: string; help: string }[] = [
  { id: "spectrum", label: "Spectrum", help: "Bars rise from the bottom, bass on the left" },
  { id: "mirror", label: "Mirror", help: "Bars from the middle outwards, bass in the middle" },
  { id: "ripple", label: "Ripple", help: "Rings spread from the centre with the loudness" },
  { id: "pulse", label: "Pulse", help: "The whole board pulses with the bass" },
];

const SWATCHES = ["#ff0000", "#ff8000", "#ffff00", "#00ff00", "#0000ff", "#00ffff", "#ff00ff", "#ffffff"];
const STORE = "aula-studio.audio";
const DEFAULTS: AudioSettings = { mode: "spectrum", gain: 1, smoothness: 4, colorful: true, color: [0, 255, 0], device: null };
const DEFAULT_DEVICE = "__default__";

function loadSettings(): AudioSettings {
  try {
    return { ...DEFAULTS, ...JSON.parse(localStorage.getItem(STORE) ?? "{}") };
  } catch {
    return DEFAULTS;
  }
}

export function AudioPage({ catalog }: { catalog: Catalog }) {
  const [settings, setSettings] = useState<AudioSettings>(loadSettings);
  const [running, setRunning] = useState(false);
  const [starting, setStarting] = useState(false);
  const [frame, setFrame] = useState<AudioFrame | null>(null);
  const [devices, setDevices] = useState<{ devices: string[]; default: string | null }>({ devices: [], default: null });
  const first = useRef(true);

  useEffect(() => {
    api.audioRunning().then(setRunning);
    api.audioDevices().then(setDevices).catch(() => {});
    const offs = [
      listen<AudioFrame>("audio-frame", (e) => setFrame(e.payload)),
      listen<string>("audio-error", (e) => toast.error("Audio lighting", { description: e.payload })),
      listen("audio-stopped", () => {
        setRunning(false);
        setFrame(null);
      }),
    ];
    return () => offs.forEach((p) => p.then((off) => off()));
  }, []);

  // settings changes reach the running stream at once, and are remembered
  useEffect(() => {
    try {
      localStorage.setItem(STORE, JSON.stringify(settings));
    } catch {
      /* storage unavailable: settings just aren't remembered */
    }
    if (first.current) {
      first.current = false;
      return;
    }
    if (running) api.audioUpdate(settings).catch(() => {});
  }, [settings]); // eslint-disable-line react-hooks/exhaustive-deps

  const toggle = async (on: boolean) => {
    setStarting(true);
    try {
      if (on) {
        await api.audioStart(settings);
        setRunning(true);
      } else {
        await api.audioStop();
        setRunning(false);
        setFrame(null);
      }
    } catch (e) {
      toast.error(on ? "Can't start audio lighting" : "Can't stop audio lighting", { description: String(e) });
    } finally {
      setStarting(false);
    }
  };

  const set = (p: Partial<AudioSettings>) => setSettings((s) => ({ ...s, ...p }));
  const colours = new Map<number, Rgb>(frame?.keys ?? []);

  return (
    <div className="grid gap-4 lg:grid-cols-[220px_1fr]">
      <Card className="self-start">
        <CardHeader>
          <CardTitle>Mode</CardTitle>
        </CardHeader>
        <CardContent className="grid gap-1">
          {MODES.map((m) => (
            <Button
              key={m.id}
              variant={settings.mode === m.id ? "secondary" : "ghost"}
              className="h-auto flex-col items-start gap-0.5 py-2 text-left whitespace-normal"
              onClick={() => set({ mode: m.id })}
            >
              <span>{m.label}</span>
              <span className="text-muted-foreground text-xs font-normal">{m.help}</span>
            </Button>
          ))}
        </CardContent>
      </Card>

      <div className="grid gap-4">
        <Card>
          <CardHeader className="flex flex-row items-start justify-between gap-4">
            <div className="grid gap-1.5">
              <CardTitle className="flex items-center gap-2">
                <AudioLines className="size-5" /> The lights follow the computer's sound
              </CardTitle>
              <CardDescription>Whatever is playing, no microphone needed. Runs while this app is open.</CardDescription>
            </div>
            <div className="flex items-center gap-2 text-sm">
              <span className={cn(!running && "text-foreground font-medium")}>Off</span>
              <Switch checked={running} disabled={starting} onCheckedChange={toggle} aria-label="Audio lighting" />
              <span className={cn(running && "text-foreground font-medium")}>On</span>
            </div>
          </CardHeader>
          <CardContent className="grid gap-4">
            <Keyboard
              keys={catalog.keys}
              selected={new Set()}
              look={(i) => {
                const c = colours.get(i);
                return c ? { fill: toHex(c) } : {};
              }}
            />
            <Levels levels={frame?.levels ?? []} running={running} />
            <div className="flex flex-wrap items-center gap-3">
              <Label>Listen to</Label>
              <Select
                value={settings.device ?? DEFAULT_DEVICE}
                onValueChange={(v) => set({ device: v === DEFAULT_DEVICE ? null : v })}
                onOpenChange={(o) => o && api.audioDevices().then(setDevices).catch(() => {})}
              >
                <SelectTrigger className="w-96 max-w-full">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value={DEFAULT_DEVICE}>Windows default{devices.default ? ` (${devices.default})` : ""}</SelectItem>
                  {devices.devices.map((d) => (
                    <SelectItem key={d} value={d}>
                      {d}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            {running && frame?.silent && (
              <Alert>
                <VolumeX />
                <AlertTitle>No sound on “{frame.device}”</AlertTitle>
                <AlertDescription>
                  Play something, or pick the device your music actually plays on. With SteelSeries Sonar, music usually
                  goes to “Sonar - Media”, and your headset or speakers carry everything.
                </AlertDescription>
              </Alert>
            )}
          </CardContent>
        </Card>

        <Card>
          <CardContent className="grid gap-6 pt-6 md:grid-cols-2">
            <div className="grid gap-6">
              <Level
                label="Gain"
                value={settings.gain}
                min={0.2}
                max={4}
                step={0.1}
                text={settings.gain.toFixed(1)}
                onChange={(v) => set({ gain: v })}
              />
              <Level
                label="Smoothness"
                value={settings.smoothness}
                min={0}
                max={9}
                step={1}
                text={String(settings.smoothness)}
                onChange={(v) => set({ smoothness: v })}
              />
            </div>
            <div className="grid content-start gap-3">
              <div className="flex items-center justify-between">
                <Label htmlFor="colorful">Colourful</Label>
                <Switch id="colorful" checked={settings.colorful} onCheckedChange={(v) => set({ colorful: v })} />
              </div>
              <div className={cn("flex flex-wrap items-center gap-2", settings.colorful && "pointer-events-none opacity-40")}>
                <input
                  type="color"
                  value={toHex(settings.color)}
                  onChange={(e) => set({ color: fromHex(e.target.value) })}
                  className="h-9 w-14 cursor-pointer rounded-md border bg-transparent"
                  aria-label="Colour"
                />
                {SWATCHES.map((s) => (
                  <button
                    key={s}
                    aria-label={s}
                    onClick={() => set({ color: fromHex(s) })}
                    className={cn(
                      "size-7 rounded-full border-2 transition-transform hover:scale-110",
                      toHex(settings.color) === s ? "border-white" : "border-transparent",
                    )}
                    style={{ background: s }}
                  />
                ))}
              </div>
            </div>
          </CardContent>
        </Card>

        <p className="text-muted-foreground flex items-start gap-2 text-xs">
          <Info className="mt-0.5 size-3.5 shrink-0" />
          Over 2.4G about 14 frames a second get through. Nothing is stored on the keyboard: about 2 seconds after you
          turn this off it goes back to its own effect.
        </p>
      </div>
    </div>
  );
}

function Levels({ levels, running }: { levels: number[]; running: boolean }) {
  const bars = levels.length ? levels : new Array(11).fill(0);
  return (
    <div className="flex h-12 items-end gap-1" aria-hidden>
      {bars.map((v, i) => (
        <div key={i} className="bg-muted flex h-full flex-1 items-end overflow-hidden rounded-sm">
          <div
            className={cn("w-full rounded-sm transition-[height] duration-75", running ? "bg-primary" : "bg-muted-foreground/30")}
            style={{ height: `${Math.max(3, v * 100)}%` }}
          />
        </div>
      ))}
    </div>
  );
}

function Level(props: {
  label: string;
  value: number;
  min: number;
  max: number;
  step: number;
  text: string;
  onChange: (v: number) => void;
}) {
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
