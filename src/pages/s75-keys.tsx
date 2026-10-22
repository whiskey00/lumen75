import { useEffect, useState } from "react";
import { toast } from "sonner";
import { RotateCcw, Save } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { Keyboard } from "@/components/keyboard";
import { Entry, S75Catalog, S75Macro, api } from "@/lib/api";
import { friendly } from "@/lib/errors";
import { S75Note, useS75 } from "@/pages/s75-lighting";

type Kind = "key" | "media" | "mouse" | "macro" | "disabled";

/** How a macro key plays it. */
export const MACRO_PLAY = ["Once", "N times", "Toggle on and off", "While held", "Click to play"];

/** An S75 Pro keymap entry, in words. */
export function describe(e: Entry, c: S75Catalog, macros: S75Macro[]): string {
  const [type, a, b, d] = e;
  if (type === 0x05 && a === 0x03) return "Disabled";
  if (type === 0x02) {
    const mods = c.modifiers.filter(([, , bit]) => a & bit).map(([, label]) => label.replace(/^(Left|Right) /, ""));
    const key = c.hid.find(([, , u]) => u === b)?.[1];
    return [...mods, ...(key ? [key] : [])].join(" + ") || "Nothing";
  }
  if (type === 0x03) return c.media.find(([, u]) => u === (a | (b << 8)))?.[0] ?? `Media 0x${(a | (b << 8)).toString(16)}`;
  if (type === 0x01) return c.mouse.find(([, m]) => m[0] === a && m[1] === b)?.[0] ?? "Mouse";
  if (type === 0x06) return `Macro “${macros[a]?.name ?? a + 1}” (${MACRO_PLAY[b] ?? ""}${b === 1 ? `, ${d}×` : ""})`;
  return "Its own function";
}

export function S75KeysPage() {
  const [state, setState] = useS75();
  const [layer, setLayer] = useState(0);
  const [key, setKey] = useState<number | null>(null);
  const [kind, setKind] = useState<Kind>("key");
  const [usage, setUsage] = useState(0x04);
  const [mods, setMods] = useState(0);
  const [media, setMedia] = useState(0xcd);
  const [mouse, setMouse] = useState(0);
  const [macro, setMacro] = useState(0);
  const [play, setPlay] = useState(0);
  const [loops, setLoops] = useState(1);
  const [busy, setBusy] = useState(false);

  // show what the chosen key does now
  useEffect(() => {
    if (!state || key === null) return;
    const e = state.saved.keymap[layer].find(([i]) => i === key)?.[1];
    if (!e) return setKind("key");
    const [type, a, b, d] = e;
    if (type === 0x02) {
      setKind("key");
      setMods(a);
      setUsage(b);
    } else if (type === 0x03) {
      setKind("media");
      setMedia(a | (b << 8));
    } else if (type === 0x01) {
      setKind("mouse");
      setMouse(Math.max(0, state.catalog.mouse.findIndex(([, m]) => m[0] === a && m[1] === b)));
    } else if (type === 0x06) {
      setKind("macro");
      setMacro(a);
      setPlay(b);
      setLoops(d || 1);
    } else setKind("disabled");
  }, [key, layer, state]);

  if (!state) return null;
  const c = state.catalog;
  const map = new Map(state.saved.keymap[layer]);
  const keyName = c.keys.find((k) => k.index === key)?.label;

  const entry = (): Entry => {
    switch (kind) {
      case "key":
        // a modifier key alone goes in as its bit
        return usage >= 0xe0 && usage <= 0xe7 ? [0x02, mods | (1 << (usage - 0xe0)), 0, 0] : [0x02, mods, usage, 0];
      case "media":
        return [0x03, media & 0xff, media >> 8, 0];
      case "mouse":
        return [0x01, ...c.mouse[mouse][1], 0];
      case "macro":
        return [0x06, macro, play, Math.max(1, loops)];
      default:
        return [0x05, 0x03, 0, 0];
    }
  };

  const send = async (e: Entry | null) => {
    if (key === null) return;
    setBusy(true);
    try {
      const saved = await api.s75SetKey(layer, key, e);
      setState({ ...state, saved });
      toast.success(e ? `${keyName}: ${describe(e, c, saved.macros)}` : `${keyName}: its own function again`);
    } catch (err) {
      toast.error("Can't remap the key", { description: friendly(err) });
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="grid gap-4">
      <S75Note />
      <Card>
        <CardHeader className="flex flex-row items-start justify-between gap-4">
          <div className="grid gap-1.5">
            <CardTitle>Keys</CardTitle>
            <CardDescription>Pick a key, choose what it does, and save. Remapped keys show their new job.</CardDescription>
          </div>
          <ToggleGroup type="single" variant="outline" value={String(layer)} onValueChange={(v) => v && setLayer(Number(v))}>
            <ToggleGroupItem value="0">Base layer</ToggleGroupItem>
            <ToggleGroupItem value="1">Fn layer</ToggleGroupItem>
          </ToggleGroup>
        </CardHeader>
        <CardContent>
          <Keyboard
            keys={c.keys}
            selected={new Set(key === null ? [] : [key])}
            onKey={(i) => setKey(i)}
            look={(i) => {
              const e = map.get(i);
              return e ? { tone: e[0] === 0x05 ? "off" : "changed", sub: describe(e, c, state.saved.macros).slice(0, 12) } : {};
            }}
          />
        </CardContent>
      </Card>

      {key !== null && (
        <Card>
          <CardHeader>
            <CardTitle className="text-base">
              {keyName} on the {layer ? "Fn" : "base"} layer
            </CardTitle>
            <CardDescription>Now: {map.has(key) ? describe(map.get(key)!, c, state.saved.macros) : "its own function"}</CardDescription>
          </CardHeader>
          <CardContent className="grid gap-4">
            <ToggleGroup type="single" variant="outline" value={kind} onValueChange={(v) => v && setKind(v as Kind)} className="justify-start">
              <ToggleGroupItem value="key">Key or combo</ToggleGroupItem>
              <ToggleGroupItem value="media">Media</ToggleGroupItem>
              <ToggleGroupItem value="mouse">Mouse</ToggleGroupItem>
              <ToggleGroupItem value="macro" disabled={state.saved.macros.length === 0} title={state.saved.macros.length ? undefined : "Make a macro first"}>
                Macro
              </ToggleGroupItem>
              <ToggleGroupItem value="disabled">Disable</ToggleGroupItem>
            </ToggleGroup>

            {kind === "key" && (
              <div className="grid gap-3">
                <div className="flex flex-wrap gap-1.5">
                  {c.modifiers.map(([name, label, bit]) => (
                    <Button key={name} size="sm" variant={mods & bit ? "secondary" : "outline"} onClick={() => setMods(mods ^ bit)}>
                      {label}
                    </Button>
                  ))}
                </div>
                <Select value={String(usage)} onValueChange={(v) => setUsage(Number(v))}>
                  <SelectTrigger className="w-56">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {c.hid.map(([name, label, u]) => (
                      <SelectItem key={name} value={String(u)}>
                        {label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
            )}
            {kind === "media" && (
              <Select value={String(media)} onValueChange={(v) => setMedia(Number(v))}>
                <SelectTrigger className="w-56">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {c.media.map(([label, u]) => (
                    <SelectItem key={u} value={String(u)}>
                      {label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            )}
            {kind === "mouse" && (
              <Select value={String(mouse)} onValueChange={(v) => setMouse(Number(v))}>
                <SelectTrigger className="w-56">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {c.mouse.map(([label], i) => (
                    <SelectItem key={label} value={String(i)}>
                      {label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            )}
            {kind === "macro" && (
              <div className="flex flex-wrap items-end gap-3">
                <div className="grid gap-1.5">
                  <Label>Macro</Label>
                  <Select value={String(macro)} onValueChange={(v) => setMacro(Number(v))}>
                    <SelectTrigger className="w-48">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      {state.saved.macros.map((m, i) => (
                        <SelectItem key={i} value={String(i)}>
                          {m.name}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </div>
                <div className="grid gap-1.5">
                  <Label>Plays</Label>
                  <Select value={String(play)} onValueChange={(v) => setPlay(Number(v))}>
                    <SelectTrigger className="w-44">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      {MACRO_PLAY.map((p, i) => (
                        <SelectItem key={p} value={String(i)}>
                          {p}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </div>
                {play === 1 && (
                  <div className="grid gap-1.5">
                    <Label htmlFor="loops">Times</Label>
                    <Input id="loops" type="number" min={1} max={255} className="w-20" value={loops} onChange={(e) => setLoops(Number(e.target.value))} />
                  </div>
                )}
              </div>
            )}

            <div className="flex gap-2">
              <Button disabled={busy} onClick={() => send(entry())}>
                <Save /> Save
              </Button>
              <Button variant="ghost" disabled={busy || !map.has(key)} onClick={() => send(null)}>
                <RotateCcw /> Its own function
              </Button>
            </div>
          </CardContent>
        </Card>
      )}
    </div>
  );
}
