import { useEffect, useMemo, useState } from "react";
import { Ban, Check, RotateCcw } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Label } from "@/components/ui/label";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { Keyboard } from "@/components/keyboard";
import { api, Board, Catalog, Entry, sameEntry } from "@/lib/api";
import { describeEntry, shortEntry } from "@/lib/keymap";
import type { Run } from "@/lib/run";

type Kind = "key" | "modifier" | "macro";

export function KeysPage({ board, catalog, busy, run }: { board: Board; catalog: Catalog; busy: boolean; run: Run }) {
  const [layer, setLayer] = useState(0);
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const entries = board.keymap[layer];
  const factory = board.factory_keymap[layer];
  const changed = (i: number) => !sameEntry(entries[i], factory[i]);
  const changedKeys = catalog.keys.filter((k) => changed(k.index));
  const only = selected.size === 1 ? [...selected][0] : null;
  const keyLabel = (i: number) => catalog.keys.find((k) => k.index === i)?.label ?? `#${i}`;

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
            <CardTitle>Key mapping</CardTitle>
            <CardDescription>
              Highlighted keys differ from the factory mapping. The Fn layer is what a key does while Fn is held.
            </CardDescription>
          </div>
          <ToggleGroup
            type="single"
            variant="outline"
            value={String(layer)}
            onValueChange={(v) => {
              if (v) {
                setLayer(Number(v));
                setSelected(new Set());
              }
            }}
          >
            <ToggleGroupItem value="0">Base layer</ToggleGroupItem>
            <ToggleGroupItem value="1">Fn layer</ToggleGroupItem>
          </ToggleGroup>
        </CardHeader>
        <CardContent>
          <Keyboard
            keys={catalog.keys}
            selected={selected}
            onKey={onKey}
            look={(i) => {
              const e = entries[i];
              if (!e) return {};
              const d = describeEntry(e, catalog, board.macros);
              if (d.kind === "none" && changed(i)) return { tone: "off", sub: "off" };
              if (changed(i)) return { tone: "changed", sub: shortEntry(e, catalog, board.macros) };
              return layer === 1 && !sameEntry(e, board.keymap[0][i]) ? { sub: shortEntry(e, catalog, board.macros) } : {};
            }}
          />
        </CardContent>
      </Card>

      {only !== null ? (
        <KeyEditor
          key={`${layer}-${only}`}
          layer={layer}
          index={only}
          label={keyLabel(only)}
          entry={entries[only]}
          factory={factory[only]}
          board={board}
          catalog={catalog}
          busy={busy}
          run={run}
        />
      ) : selected.size > 1 ? (
        <Card>
          <CardContent className="flex flex-wrap items-center gap-3 pt-6">
            <span className="text-sm">{selected.size} keys selected</span>
            <div className="ml-auto flex gap-2">
              <Button
                variant="outline"
                disabled={busy}
                onClick={() => run(`Disable ${selected.size} keys`, () => api.setKeys(layer, [...selected].map((i) => [i, [0, 0, 0, 0] as Entry])))}
              >
                <Ban /> Disable
              </Button>
              <Button variant="outline" disabled={busy} onClick={() => run(`Reset ${selected.size} keys`, () => api.resetKeys(layer, [...selected]))}>
                <RotateCcw /> Reset to factory
              </Button>
            </div>
          </CardContent>
        </Card>
      ) : (
        <Card>
          <CardContent className="text-muted-foreground pt-6 text-sm">Select a key to change what it does.</CardContent>
        </Card>
      )}

      {changedKeys.length > 0 && (
        <Card>
          <CardHeader className="flex flex-row items-center justify-between">
            <CardTitle className="text-base">Changed on the {layer ? "Fn" : "base"} layer</CardTitle>
            <Button
              variant="ghost"
              size="sm"
              disabled={busy}
              onClick={() => run("Reset layer", () => api.resetKeys(layer, changedKeys.map((k) => k.index)))}
            >
              <RotateCcw /> Reset all
            </Button>
          </CardHeader>
          <CardContent className="flex flex-wrap gap-2">
            {changedKeys.map((k) => (
              <Badge key={k.index} variant="secondary" className="cursor-pointer" onClick={() => setSelected(new Set([k.index]))}>
                {k.label} → {describeEntry(entries[k.index], catalog, board.macros).text}
              </Badge>
            ))}
          </CardContent>
        </Card>
      )}
    </div>
  );
}

function KeyEditor(props: {
  layer: number;
  index: number;
  label: string;
  entry: Entry;
  factory: Entry;
  board: Board;
  catalog: Catalog;
  busy: boolean;
  run: Run;
}) {
  const { layer, index, label, entry, factory, board, catalog, busy, run } = props;
  const now = describeEntry(entry, catalog, board.macros);
  const fac = describeEntry(factory, catalog, board.macros);
  const isFactory = sameEntry(entry, factory);

  const initialKind: Kind = entry[0] === 3 ? "macro" : entry[0] === 0 && !entry[3] && entry[1] ? "modifier" : "key";
  const [kind, setKind] = useState<Kind>(initialKind);
  const [usage, setUsage] = useState(entry[0] === 0 && entry[3] ? entry[3] : catalog.hid[0][2]);
  const [mods, setMods] = useState<string[]>(
    entry[0] === 0 ? catalog.modifiers.filter(([, , b]) => entry[1] & b).map(([n]) => n) : [],
  );
  const [modifier, setModifier] = useState(
    catalog.modifiers.find(([, , b]) => entry[0] === 0 && !entry[3] && entry[1] === b)?.[0] ?? "lctrl",
  );
  const [macro, setMacro] = useState(entry[0] === 3 ? entry[3] : 0);
  const [mode, setMode] = useState(entry[0] === 3 ? entry[1] : 1);
  const [count, setCount] = useState(entry[0] === 3 ? entry[2] : 1);
  useEffect(() => setKind(initialKind), [entry.join()]); // eslint-disable-line react-hooks/exhaustive-deps

  const target = useMemo((): Entry | null => {
    if (kind === "key") {
      const bits = catalog.modifiers.filter(([n]) => mods.includes(n)).reduce((a, [, , b]) => a | b, 0);
      return [0, bits, 0, usage];
    }
    if (kind === "modifier") {
      const bit = catalog.modifiers.find(([n]) => n === modifier)?.[2] ?? 1;
      return [0, bit, 0, 0];
    }
    if (!board.macros.length) return null;
    return [3, mode, mode === 1 ? Math.max(1, Math.min(255, count)) : 1, macro];
  }, [kind, usage, mods, modifier, macro, mode, count, board.macros.length, catalog.modifiers]);

  const preview = target ? describeEntry(target, catalog, board.macros).text : "";

  return (
    <Card>
      <CardHeader className="flex flex-row items-start justify-between gap-4">
        <div className="grid gap-1.5">
          <CardTitle>
            {layer ? "Fn + " : ""}
            {label}
          </CardTitle>
          <CardDescription>
            Now: <span className="text-foreground">{now.text}</span>
            {!isFactory && <> · factory: {fac.text}</>}
          </CardDescription>
        </div>
        <div className="flex gap-2">
          <Button variant="outline" size="sm" disabled={busy || now.kind === "none"} onClick={() => run(`Disable ${label}`, () => api.setKeys(layer, [[index, [0, 0, 0, 0]]]))}>
            <Ban /> Disable
          </Button>
          <Button variant="outline" size="sm" disabled={busy || isFactory} onClick={() => run(`Reset ${label}`, () => api.resetKeys(layer, [index]))}>
            <RotateCcw /> Factory
          </Button>
        </div>
      </CardHeader>
      <CardContent>
        <Tabs value={kind} onValueChange={(v) => setKind(v as Kind)}>
          <TabsList>
            <TabsTrigger value="key">Key</TabsTrigger>
            <TabsTrigger value="modifier">Modifier</TabsTrigger>
            <TabsTrigger value="macro">Macro</TabsTrigger>
          </TabsList>
          <TabsContent value="key" className="grid gap-4 pt-3">
            <div className="grid gap-2">
              <Label>Sends</Label>
              <Select value={String(usage)} onValueChange={(v) => setUsage(Number(v))}>
                <SelectTrigger className="w-60">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {catalog.hid.map(([n, l, u]) => (
                    <SelectItem key={n} value={String(u)}>
                      {l}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div className="grid gap-2">
              <Label>Together with (optional)</Label>
              <ToggleGroup type="multiple" variant="outline" value={mods} onValueChange={setMods} className="flex-wrap justify-start">
                {catalog.modifiers.map(([n, l]) => (
                  <ToggleGroupItem key={n} value={n} size="sm">
                    {l}
                  </ToggleGroupItem>
                ))}
              </ToggleGroup>
            </div>
          </TabsContent>
          <TabsContent value="modifier" className="grid gap-2 pt-3">
            <Label>Acts as</Label>
            <Select value={modifier} onValueChange={setModifier}>
              <SelectTrigger className="w-60">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {catalog.modifiers.map(([n, l]) => (
                  <SelectItem key={n} value={n}>
                    {l}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </TabsContent>
          <TabsContent value="macro" className="grid gap-4 pt-3">
            {board.macros.length === 0 ? (
              <p className="text-muted-foreground text-sm">No macros on the keyboard yet. Create one on the Macros page.</p>
            ) : (
              <div className="flex flex-wrap items-end gap-4">
                <div className="grid gap-2">
                  <Label>Macro</Label>
                  <Select value={String(macro)} onValueChange={(v) => setMacro(Number(v))}>
                    <SelectTrigger className="w-52">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      {board.macros.map((m, i) => (
                        <SelectItem key={i} value={String(i)}>
                          {m.name}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </div>
                <div className="grid gap-2">
                  <Label>Runs</Label>
                  <Select value={String(mode)} onValueChange={(v) => setMode(Number(v))}>
                    <SelectTrigger className="w-48">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="1">a number of times</SelectItem>
                      <SelectItem value="2">while held</SelectItem>
                      <SelectItem value="4">until a key is pressed</SelectItem>
                    </SelectContent>
                  </Select>
                </div>
                {mode === 1 && (
                  <div className="grid gap-2">
                    <Label htmlFor="count">Times</Label>
                    <Input id="count" type="number" min={1} max={255} value={count} onChange={(e) => setCount(Number(e.target.value))} className="w-24" />
                  </div>
                )}
              </div>
            )}
          </TabsContent>
        </Tabs>
        <div className="mt-6 flex items-center justify-end gap-3">
          {target && <span className="text-muted-foreground text-sm">Will send: {preview}</span>}
          <Button
            disabled={busy || !target || sameEntry(target, entry)}
            onClick={() => target && run(`${label} → ${preview}`, () => api.setKeys(layer, [[index, target]]))}
          >
            <Check /> Apply
          </Button>
        </div>
      </CardContent>
    </Card>
  );
}
