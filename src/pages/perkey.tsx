import { useEffect, useMemo, useState } from "react";
import { Eraser, PaintBucket, Paintbrush, Undo2, Upload } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Keyboard } from "@/components/keyboard";
import { cn } from "@/lib/utils";
import { api, Board, Catalog, fromHex, Rgb, toHex } from "@/lib/api";
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
  useEffect(() => setDraft(board.custom), [board.custom]);

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
          <input
            type="color"
            value={colour}
            onChange={(e) => setColour(e.target.value)}
            className="h-9 w-14 cursor-pointer rounded-md border bg-transparent"
            aria-label="Colour"
          />
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
