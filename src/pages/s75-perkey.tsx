import { useEffect, useMemo, useState } from "react";
import { toast } from "sonner";
import { Download, Eraser, PaintBucket, Paintbrush, Undo2, Upload } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { Slider } from "@/components/ui/slider";
import { ColourInput } from "@/components/colour-input";
import { Keyboard } from "@/components/keyboard";
import { cn } from "@/lib/utils";
import { api, fromHex, Rgb, S75Saved, toHex } from "@/lib/api";
import { friendly } from "@/lib/errors";
import { useS75 } from "@/pages/s75-lighting";

const SWATCHES = ["#ff0000", "#ff6000", "#ffd000", "#00ff40", "#00e5ff", "#0040ff", "#8a00ff", "#ff00b0", "#ffffff"];

export function S75PerKeyPage() {
  const [state, setState] = useS75();
  const [draft, setDraft] = useState<Rgb[] | null>(null);
  const [brightness, setBrightness] = useState(5);
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [colour, setColour] = useState("#ff0000");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (state && draft === null) {
      setDraft(state.saved.perkey);
      setBrightness(state.saved.perkey_brightness);
    }
  }, [state, draft]);

  const changed = useMemo(
    () => (state && draft ? draft.filter((c, i) => c.join() !== state.saved.perkey[i]?.join()).length : 0),
    [draft, state],
  );

  if (!state || !draft) return null;
  const keys = state.catalog.keys;

  const paint = (rgb: Rgb, which: Iterable<number>) => {
    const next = draft.slice();
    for (const i of which) next[i] = rgb;
    setDraft(next);
  };

  const run = async (what: string, f: () => Promise<S75Saved>) => {
    setBusy(true);
    try {
      const saved = await f();
      setState({ ...state, saved });
      setDraft(saved.perkey);
      toast.success(what);
    } catch (e) {
      toast.error(what, { description: friendly(e) });
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="grid gap-4">
      <Card>
        <CardHeader className="flex flex-row items-start justify-between gap-4">
          <div className="grid gap-1.5">
            <CardTitle>Per-key colours</CardTitle>
            <CardDescription>
              Select keys (hold and drag over several, Ctrl/Shift to add more), paint them, then apply. Applying switches the
              S75 Pro to its per-key mode.
            </CardDescription>
          </div>
          <Button
            variant="outline"
            size="sm"
            disabled={busy || state.connection !== "USB cable"}
            title={state.connection === "USB cable" ? "Load the colours stored on the keyboard" : "Reading needs the USB cable"}
            onClick={() => run("Read from the keyboard", api.s75ReadPerkey)}
          >
            <Download /> Read from keyboard
          </Button>
        </CardHeader>
        <CardContent className="grid gap-4">
          <Keyboard
            keys={keys}
            selected={selected}
            onSelect={setSelected}
            look={(i) => {
              const c = draft[i];
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
                <PaintBucket /> Fill all
              </Button>
            </div>
          </div>
          <div className="grid max-w-sm gap-3">
            <div className="flex items-center justify-between text-sm">
              <Label>Brightness</Label>
              <span className="text-muted-foreground tabular-nums">{brightness} / 5</span>
            </div>
            <Slider min={0} max={5} step={1} value={[brightness]} onValueChange={([v]) => setBrightness(v)} />
          </div>
        </CardContent>
      </Card>

      <div
        className={cn(
          "flex flex-wrap items-center gap-3 rounded-lg border px-4 py-3",
          changed > 0 && "bg-background/95 border-primary/40 sticky bottom-0 z-10 shadow-lg backdrop-blur",
        )}
      >
        <span className="text-sm">
          {changed > 0 ? `${changed} ${changed === 1 ? "key" : "keys"} changed, not on the keyboard yet` : "Send these colours to the keyboard."}
        </span>
        <div className="ml-auto flex gap-2">
          <Button variant="ghost" disabled={busy || changed === 0} onClick={() => setDraft(state.saved.perkey)}>
            <Undo2 /> Discard
          </Button>
          <Button disabled={busy} onClick={() => run("Per-key colours", () => api.s75SetPerkey(draft, brightness))}>
            <Upload /> Apply to keyboard
          </Button>
        </div>
      </div>
    </div>
  );
}
