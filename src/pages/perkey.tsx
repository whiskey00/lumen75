import { useEffect, useMemo, useState } from "react";
import { Eraser, PaintBucket, Paintbrush, Undo2, Upload } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { toast } from "sonner";
import { Keyboard } from "@/components/keyboard";
import { ColourInput } from "@/components/colour-input";
import { PresetBar } from "@/components/preset-bar";
import { cn } from "@/lib/utils";
import { api, Board, Catalog, fromHex, Presets, Rgb, toHex } from "@/lib/api";

/** The most used colours of a layout (ignoring keys that are off), for the preset chip. */
function mainColours(colors: Rgb[]): string[] {
  const count = new Map<string, number>();
  for (const c of colors) if (c.some((v) => v)) count.set(toHex(c), (count.get(toHex(c)) ?? 0) + 1);
  const top = [...count.entries()].sort((a, b) => b[1] - a[1]).map(([c]) => c);
  return top.length ? top.slice(0, 5) : ["#000000"];
}
import type { Run } from "@/lib/run";

const SWATCHES = ["#ff0000", "#ff6000", "#ffd000", "#00ff40", "#00e5ff", "#0040ff", "#8a00ff", "#ff00b0", "#ffffff"];

const GROUPS: [string, string[]][] = [
  ["Letters", "qwertyuiopasdfghjklzxcvbnm".split("")],
  ["WASD", ["w", "a", "s", "d"]],
  ["Arrows", ["up", "down", "left", "right"]],
  ["Numbers", ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"]],
  ["F-keys", ["f1", "f2", "f3", "f4", "f5", "f6", "f7", "f8", "f9", "f10", "f11", "f12"]],
  ["Modifiers", ["lshift", "rshift", "lctrl", "lwin", "lalt", "ralt", "fn", "capslock", "tab"]],
];

export function PerKeyPage({ board, catalog, busy, run }: { board: Board; catalog: Catalog; busy: boolean; run: Run }) {
  const [draft, setDraft] = useState<Rgb[]>(board.custom);
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [colour, setColour] = useState("#ff0000");
  const [mine, setMine] = useState<Presets["perkey"]>([]);
  useEffect(() => setDraft(board.custom), [board.custom]);
  useEffect(() => {
    api.presetsGet().then((p) => setMine(p.perkey)).catch(() => {});
  }, []);

  const dirty = useMemo(() => draft.some((c, i) => c.join() !== board.custom[i]?.join()), [draft, board.custom]);
  const keys = catalog.keys;
  const byName = (n: string) => keys.find((k) => k.name === n)?.index;

  const paint = (rgb: Rgb, indices: Iterable<number>) =>
    setDraft((d) => {
      const n = d.slice();
      for (const i of indices) n[i] = rgb;
      return n;
    });

  const onKey = (index: number, additive: boolean) =>
    setSelected((prev) => {
      const next = new Set(additive ? prev : []);
      if (additive && prev.has(index)) next.delete(index);
      else next.add(index);
      return next;
    });

  return (
    <div className="grid gap-4">
      <Card>
        <CardHeader className="flex flex-row items-start justify-between gap-4">
          <div className="grid gap-1.5">
            <CardTitle>Per-key colours</CardTitle>
            <CardDescription>
              Select keys (Ctrl/Shift-click for more), paint them, then apply. Applying switches the keyboard to the per-key effect.
            </CardDescription>
          </div>
          {dirty && <Badge variant="secondary">Not applied yet</Badge>}
        </CardHeader>
        <CardContent className="grid gap-4">
          <Keyboard
            keys={keys}
            selected={selected}
            onKey={onKey}
            look={(i) => {
              const c = draft[i];
              return c && c.some((v) => v) ? { fill: toHex(c) } : {};
            }}
          />
          <div className="flex flex-wrap items-center gap-2 text-sm">
            <span className="text-muted-foreground mr-1">Select:</span>
            <Button size="sm" variant="outline" onClick={() => setSelected(new Set(keys.map((k) => k.index)))}>
              All
            </Button>
            {GROUPS.map(([label, names]) => (
              <Button
                key={label}
                size="sm"
                variant="outline"
                onClick={() => setSelected(new Set(names.map(byName).filter((i): i is number => i !== undefined)))}
              >
                {label}
              </Button>
            ))}
            <Button size="sm" variant="ghost" disabled={selected.size === 0} onClick={() => setSelected(new Set())}>
              Clear selection
            </Button>
            <span className="text-muted-foreground ml-auto">{selected.size} selected</span>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardContent className="flex flex-wrap items-center gap-3 pt-6">
          <ColourInput value={colour} onChange={setColour} label="Colour" />
          <div className="flex gap-1.5">
            {SWATCHES.map((s) => (
              <button
                key={s}
                aria-label={s}
                onClick={() => setColour(s)}
                className={cn("size-7 rounded-full border-2 transition-transform hover:scale-110", s === colour ? "border-white" : "border-transparent")}
                style={{ background: s }}
              />
            ))}
          </div>
          <div className="ml-auto flex flex-wrap gap-2">
            <Button variant="secondary" disabled={selected.size === 0} onClick={() => paint(fromHex(colour), selected)}>
              <Paintbrush /> Paint selected
            </Button>
            <Button variant="outline" disabled={selected.size === 0} onClick={() => paint([0, 0, 0], selected)}>
              <Eraser /> Turn off
            </Button>
            <Button variant="outline" onClick={() => paint(fromHex(colour), keys.map((k) => k.index))}>
              <PaintBucket /> Fill all
            </Button>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardContent className="pt-6">
          <PresetBar
            title="My layouts"
            saves="the colours of every key as they are in the editor now (also changes you haven't applied)"
            items={mine.map((p) => ({ name: p.name, swatch: mainColours(p.colors) }))}
            onLoad={(name) => {
              const p = mine.find((x) => x.name === name);
              if (p) {
                setDraft(p.colors);
                toast.info(`Loaded “${name}”`, { description: "Click “Apply to keyboard” to send it." });
              }
            }}
            onSave={async (name) => {
              try {
                setMine((await api.perkeyPresetSave(name, draft)).perkey);
                toast.success(`Saved “${name.trim()}”`);
                return true;
              } catch (e) {
                toast.error("Can't save the layout", { description: String(e) });
                return false;
              }
            }}
            onDelete={async (name) => {
              setMine((await api.presetDelete("perkey", name)).perkey);
            }}
          />
        </CardContent>
      </Card>

      <div className="flex justify-end gap-2">
        <Button variant="ghost" disabled={!dirty || busy} onClick={() => setDraft(board.custom)}>
          <Undo2 /> Revert
        </Button>
        <Button disabled={busy || (!dirty && board.active_effect === 32)} onClick={() => run("Per-key colours", () => api.setCustom(draft))}>
          <Upload /> Apply to keyboard
        </Button>
      </div>
    </div>
  );
}
