import { useEffect, useState } from "react";
import { Palette, RotateCcw, Sparkles } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Slider } from "@/components/ui/slider";
import { Badge } from "@/components/ui/badge";
import { ColourInput } from "@/components/colour-input";
import { cn } from "@/lib/utils";
import { api, Board, Catalog, EffectView, fromHex, toHex } from "@/lib/api";
import type { Run } from "@/lib/run";

const MULTI = 7;

export function LightingPage({ board, catalog, busy, run }: { board: Board; catalog: Catalog; busy: boolean; run: Run }) {
  const [hw, setHw] = useState(board.active_effect);
  useEffect(() => setHw(board.active_effect), [board.active_effect]);
  const fx = board.effects.find((e) => e.hw === hw);

  return (
    <div className="grid gap-4">
      <Card>
        <CardHeader>
          <CardTitle>Effect</CardTitle>
          <CardDescription>Click an effect to switch the keyboard to it.</CardDescription>
        </CardHeader>
        <CardContent className="grid grid-cols-2 gap-2 sm:grid-cols-4 lg:grid-cols-6">
          {board.effects.map((e) => (
            <Button
              key={e.hw}
              variant={board.active_effect === e.hw ? "default" : "outline"}
              className={cn("h-auto justify-start py-2.5", hw === e.hw && board.active_effect !== e.hw && "ring-2 ring-ring")}
              disabled={busy}
              onClick={() => {
                setHw(e.hw);
                if (e.hw !== board.active_effect) run(`Effect: ${e.label}`, () => api.setEffect({ hw: e.hw }));
              }}
            >
              <EffectSwatch e={e} />
              {e.label}
            </Button>
          ))}
        </CardContent>
      </Card>

      {fx && fx.name === "custom" && (
        <Card>
          <CardContent className="text-muted-foreground flex items-center gap-2 py-6 text-sm">
            <Sparkles className="size-4" /> The per-key effect shows the colours from the Per-key page.
          </CardContent>
        </Card>
      )}

      {fx && fx.slot && <EffectSettings key={fx.hw} fx={fx} catalog={catalog} busy={busy} run={run} />}
    </div>
  );
}

function EffectSwatch({ e }: { e: EffectView }) {
  if (!e.slot || !e.color || !e.palette || e.slot.color === MULTI) {
    return <span className="size-3 shrink-0 rounded-full bg-[conic-gradient(red,yellow,lime,cyan,blue,magenta,red)]" />;
  }
  return <span className="size-3 shrink-0 rounded-full border" style={{ background: toHex(e.palette[e.slot.color]) }} />;
}

function EffectSettings({ fx, catalog, busy, run }: { fx: EffectView; catalog: Catalog; busy: boolean; run: Run }) {
  const slot = fx.slot!;
  const [brightness, setBrightness] = useState(slot.brightness);
  const [speed, setSpeed] = useState(slot.speed);
  useEffect(() => {
    setBrightness(slot.brightness);
    setSpeed(slot.speed);
  }, [slot.brightness, slot.speed]);

  const current = slot.color < MULTI && fx.palette ? fx.palette[slot.color] : null;
  const [picked, setPicked] = useState(current ? toHex(current) : "#ff6000");
  useEffect(() => {
    if (current) setPicked(toHex(current));
  }, [current?.join()]); // eslint-disable-line react-hooks/exhaustive-deps

  const factory = fx.palette?.every((c, i) => toHex(c) === ["#ff0000", "#00ff00", "#0000ff", "#ffff00", "#ff00ff", "#00ffff", "#ffffff"][i]);

  return (
    <Card>
      <CardHeader>
        <CardTitle>{fx.label}</CardTitle>
        <CardDescription>Changes are written to the keyboard as you make them.</CardDescription>
      </CardHeader>
      <CardContent className="grid gap-6">
        <Level
          label="Brightness"
          value={brightness}
          max={9}
          disabled={busy}
          onChange={setBrightness}
          onCommit={(v) => run(`Brightness ${v}`, () => api.setEffect({ hw: fx.hw, brightness: v }))}
        />
        {fx.speed && (
          <Level
            label="Speed"
            value={speed}
            max={4}
            disabled={busy}
            onChange={setSpeed}
            onCommit={(v) => run(`Speed ${v}`, () => api.setEffect({ hw: fx.hw, speed: v }))}
          />
        )}

        {fx.color && fx.palette && (
          <div className="grid gap-3">
            <div className="flex items-center justify-between">
              <span className="text-sm font-medium">Colour</span>
              {!factory && (
                <Badge variant="secondary" className="gap-1">
                  <Palette className="size-3" /> custom colours
                </Badge>
              )}
            </div>
            <div className="flex flex-wrap items-center gap-2">
              {fx.palette.map((c, i) => (
                <button
                  key={i}
                  title={catalog.colors[i]}
                  disabled={busy}
                  onClick={() => run(`Colour: ${catalog.colors[i]}`, () => api.setEffect({ hw: fx.hw, color: i }))}
                  className={cn(
                    "size-8 rounded-full border-2 transition-transform hover:scale-110 disabled:opacity-50",
                    slot.color === i ? "border-white ring-2 ring-ring" : "border-transparent",
                  )}
                  style={{ background: toHex(c) }}
                />
              ))}
              <button
                title="multicolour"
                disabled={busy}
                onClick={() => run("Colour: multicolour", () => api.setEffect({ hw: fx.hw, color: MULTI }))}
                className={cn(
                  "size-8 rounded-full border-2 bg-[conic-gradient(red,yellow,lime,cyan,blue,magenta,red)] transition-transform hover:scale-110 disabled:opacity-50",
                  slot.color === MULTI ? "border-white ring-2 ring-ring" : "border-transparent",
                )}
              />
            </div>
            <div className="flex flex-wrap items-center gap-3">
              <ColourInput value={picked} onChange={setPicked} label="Any colour" />
              <Button
                variant="secondary"
                disabled={busy}
                onClick={() => run(`Colour ${picked}`, () => api.setEffect({ hw: fx.hw, rgb: fromHex(picked) }))}
              >
                Use this colour
              </Button>
              {!factory && (
                <Button variant="ghost" disabled={busy} onClick={() => run("Factory colours", () => api.resetPalette(fx.hw))}>
                  <RotateCcw /> Factory colours
                </Button>
              )}
              <span className="text-muted-foreground text-xs">
                Replaces {slot.color === MULTI ? "the first" : "the selected"} colour of this effect with any colour.
              </span>
            </div>
          </div>
        )}
      </CardContent>
    </Card>
  );
}

function Level(props: {
  label: string;
  value: number;
  max: number;
  disabled: boolean;
  onChange: (v: number) => void;
  onCommit: (v: number) => void;
}) {
  return (
    <div className="grid gap-3">
      <div className="flex items-center justify-between text-sm">
        <span className="font-medium">{props.label}</span>
        <span className="text-muted-foreground tabular-nums">
          {props.value} / {props.max}
        </span>
      </div>
      <Slider
        min={0}
        max={props.max}
        step={1}
        value={[props.value]}
        disabled={props.disabled}
        onValueChange={([v]) => props.onChange(v)}
        onValueCommit={([v]) => props.onCommit(v)}
      />
    </div>
  );
}
