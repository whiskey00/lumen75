import { useEffect, useRef, useState } from "react";
import { toast } from "sonner";
import { ArrowDown, ArrowLeft, ArrowRight, ArrowUp, Info } from "lucide-react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { Slider } from "@/components/ui/slider";
import { Switch } from "@/components/ui/switch";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { ColourInput } from "@/components/colour-input";
import { api, fromHex, S75Lighting, S75State, toHex } from "@/lib/api";
import { friendly } from "@/lib/errors";

/** Control bits of an effect: which settings it uses. */
const CAP = { brightness: 1, speed: 2, leftRight: 4, upDown: 8, colourful: 0x10, colour: 0x20 };

/** On the S75 Pro pages whose settings the keyboard can't report back. */
export function S75Note() {
  return (
    <Alert>
      <Info />
      <AlertTitle>Shows what Lumen75 last set</AlertTitle>
      <AlertDescription>
        The S75 Pro can't report these settings back, so changes made elsewhere (its own shortcuts or another app) don't show
        here.
      </AlertDescription>
    </Alert>
  );
}

/** The S75 Pro's state, loaded once. */
export function useS75() {
  const [state, setState] = useState<S75State | null>(null);
  useEffect(() => {
    api.s75Get().then(setState).catch(() => {});
  }, []);
  return [state, setState] as const;
}

export function S75LightingPage() {
  const [state, setState] = useS75();
  const [busy, setBusy] = useState(false);
  const pending = useRef<number | null>(null);

  if (!state) return null;
  const l = state.saved.lighting;
  const effect = state.catalog.effects.find(([m]) => m === l.mode);
  const caps = effect?.[2] ?? 0;

  /** Change the lighting: shown at once, sent shortly after (sliders send many changes). */
  const set = (p: Partial<S75Lighting>) => {
    const next = { ...l, ...p };
    setState({ ...state, saved: { ...state.saved, lighting: next } });
    if (pending.current) window.clearTimeout(pending.current);
    pending.current = window.setTimeout(async () => {
      setBusy(true);
      try {
        const saved = await api.s75SetLighting(next);
        setState((s) => (s ? { ...s, saved } : s));
      } catch (e) {
        toast.error("Can't change the lighting", { description: friendly(e) });
      } finally {
        setBusy(false);
      }
    }, 300);
  };

  const setBar = async (mode: number) => {
    try {
      const saved = await api.s75SetBar(mode);
      setState({ ...state, saved });
    } catch (e) {
      toast.error("Can't change the light bar", { description: friendly(e) });
    }
  };

  const dirs = [
    ...(caps & CAP.leftRight ? [{ v: 0, icon: ArrowLeft }, { v: 1, icon: ArrowRight }] : []),
    ...(caps & CAP.upDown ? [{ v: 2, icon: ArrowUp }, { v: 3, icon: ArrowDown }] : []),
  ];

  return (
    <div className="grid gap-4">
      <S75Note />
      <Card>
        <CardHeader>
          <CardTitle>Effect</CardTitle>
          <CardDescription>The S75 Pro's own effects, stored on the keyboard.{busy && " Sending…"}</CardDescription>
        </CardHeader>
        <CardContent className="grid grid-cols-2 gap-2 sm:grid-cols-4 lg:grid-cols-5">
          {state.catalog.effects.map(([mode, name]) => (
            <Button key={mode} variant={mode === l.mode ? "secondary" : "outline"} onClick={() => set({ mode })}>
              {name}
            </Button>
          ))}
        </CardContent>
      </Card>

      {l.mode !== 0 && (
        <Card>
          <CardContent className="grid gap-6 pt-6 md:grid-cols-2">
            {caps & CAP.colour ? (
              <div className="grid gap-3">
                <Label>Colour</Label>
                <div className="flex items-center gap-3">
                  <ColourInput label="Colour" value={toHex(l.rgb)} onChange={(h) => set({ rgb: fromHex(h), colorful: false })} />
                  {caps & CAP.colourful ? (
                    <div className="flex items-center gap-2 text-sm">
                      <Switch checked={l.colorful} onCheckedChange={(colorful) => set({ colorful })} aria-label="Rainbow colours" />
                      Rainbow
                    </div>
                  ) : null}
                </div>
              </div>
            ) : null}
            {caps & CAP.brightness ? <Level label="Brightness" value={l.brightness} onChange={(brightness) => set({ brightness })} /> : null}
            {caps & CAP.speed ? <Level label="Speed" value={l.speed} onChange={(speed) => set({ speed })} /> : null}
            {dirs.length > 0 && (
              <div className="grid gap-3">
                <Label>Direction</Label>
                <ToggleGroup type="single" variant="outline" value={String(l.direction)} onValueChange={(v) => v && set({ direction: Number(v) })} className="justify-start">
                  {dirs.map((d) => (
                    <ToggleGroupItem key={d.v} value={String(d.v)} aria-label={`direction ${d.v}`}>
                      <d.icon />
                    </ToggleGroupItem>
                  ))}
                </ToggleGroup>
              </div>
            )}
          </CardContent>
        </Card>
      )}

      <Card>
        <CardHeader>
          <CardTitle className="text-base">Light bar</CardTitle>
          <CardDescription>The strip of light on the side. Fn+Z turns it on and off.</CardDescription>
        </CardHeader>
        <CardContent>
          <Select value={String(state.saved.bar)} onValueChange={(v) => setBar(Number(v))}>
            <SelectTrigger className="w-48">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {state.catalog.bar_modes.map(([mode, name]) => (
                <SelectItem key={mode} value={String(mode)}>
                  {name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </CardContent>
      </Card>
    </div>
  );
}

function Level({ label, value, onChange }: { label: string; value: number; onChange: (v: number) => void }) {
  return (
    <div className="grid gap-3">
      <div className="flex items-center justify-between text-sm">
        <Label>{label}</Label>
        <span className="text-muted-foreground tabular-nums">{value} / 5</span>
      </div>
      <Slider min={0} max={5} step={1} value={[value]} onValueChange={([v]) => onChange(v)} />
    </div>
  );
}
