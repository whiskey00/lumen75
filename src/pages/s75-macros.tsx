import { useEffect, useMemo, useState } from "react";
import { toast } from "sonner";
import { Plus, Save, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { cn } from "@/lib/utils";
import { api, S75Catalog, S75Macro, S75Step } from "@/lib/api";
import { friendly } from "@/lib/errors";
import { S75Untested, useS75 } from "@/pages/s75-lighting";

const MODIFIER_USAGE: Record<string, number> = {
  ctrl: 0xe0, lctrl: 0xe0, shift: 0xe1, lshift: 0xe1, alt: 0xe2, lalt: 0xe2, win: 0xe3, lwin: 0xe3,
  rctrl: 0xe4, rshift: 0xe5, ralt: 0xe6, rwin: 0xe7,
};
const BUTTONS: Record<string, number> = { left: 1, right: 2, middle: 4 };
/** Characters `text` can type besides letters and digits (US layout): [usage, shifted]. */
const CHARS: Record<string, [number, boolean]> = {
  " ": [0x2c, false], ".": [0x37, false], ",": [0x36, false], "-": [0x2d, false], "/": [0x38, false],
  "!": [0x1e, true], "?": [0x38, true], ":": [0x33, true], ";": [0x33, false], "'": [0x34, false],
};

/**
 * Turn typed steps into the keyboard's steps, one per line:
 * `ctrl+c` (press and release), `down shift` / `up shift`, `wait 100` (ms),
 * `click left` / `mousedown left` / `mouseup left`, `text Hello` (letters, digits, simple punctuation).
 */
export function parseSteps(text: string, c: S75Catalog): { steps: S75Step[]; error: string | null } {
  const usage = (name: string) => MODIFIER_USAGE[name] ?? c.hid.find(([n]) => n === name)?.[2];
  const steps: S75Step[] = [];
  const lines = text.split("\n").map((l) => l.trim()).filter(Boolean);
  for (const [n, line] of lines.entries()) {
    const fail = (why: string) => ({ steps, error: `Line ${n + 1}: ${why}` });
    const [word, ...rest] = line.split(/\s+/);
    const arg = rest.join(" ");
    const w = word.toLowerCase();
    if (w === "wait") {
      const ms = Number(arg);
      if (!(ms >= 10 && ms <= 65535)) return fail("wait takes 10 to 65535 ms.");
      steps.push({ kind: "delay", ms });
    } else if (w === "down" || w === "up") {
      const u = usage(arg.toLowerCase());
      if (u === undefined) return fail(`unknown key “${arg}”.`);
      steps.push({ kind: w === "down" ? "key_down" : "key_up", usage: u });
    } else if (w === "click" || w === "mousedown" || w === "mouseup") {
      const b = BUTTONS[arg.toLowerCase()];
      if (!b) return fail("mouse buttons are left, right and middle.");
      if (w !== "mouseup") steps.push({ kind: "mouse_down", button: b });
      if (w !== "mousedown") steps.push({ kind: "mouse_up", button: b });
    } else if (w === "text") {
      const typed = line.slice(4).trimStart();
      for (const ch of typed) {
        const lower = ch.toLowerCase();
        const u = /[a-z0-9]/.test(lower) ? usage(lower) : CHARS[ch]?.[0];
        if (u === undefined) return fail(`can't type “${ch}”.`);
        const shift = (/[A-Z]/.test(ch)) || !!CHARS[ch]?.[1];
        if (shift) steps.push({ kind: "key_down", usage: 0xe1 });
        steps.push({ kind: "key_down", usage: u }, { kind: "key_up", usage: u });
        if (shift) steps.push({ kind: "key_up", usage: 0xe1 });
      }
    } else {
      // a key or combo, pressed and released
      const parts = line.toLowerCase().split("+").map((p) => p.trim());
      const us = parts.map(usage);
      const bad = parts.find((_, i) => us[i] === undefined);
      if (bad !== undefined) return fail(`unknown key “${bad}”.`);
      for (const u of us) steps.push({ kind: "key_down", usage: u! });
      for (const u of [...us].reverse()) steps.push({ kind: "key_up", usage: u! });
    }
  }
  return { steps, error: null };
}

export function S75MacrosPage() {
  const [state, setState] = useS75();
  const [list, setList] = useState<S75Macro[] | null>(null);
  const [cur, setCur] = useState(0);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (state && list === null) setList(state.saved.macros);
  }, [state, list]);

  const parsed = useMemo(
    () => (state && list?.[cur] ? parseSteps(list[cur].text, state.catalog) : { steps: [], error: null }),
    [state, list, cur],
  );

  if (!state || !list) return null;
  const m = list[cur];
  const dirty = JSON.stringify(list) !== JSON.stringify(state.saved.macros);

  const edit = (p: Partial<S75Macro>) => {
    const next = list.slice();
    next[cur] = { ...next[cur], ...p };
    if (p.text !== undefined) next[cur].steps = parseSteps(p.text, state.catalog).steps;
    setList(next);
  };

  const save = async () => {
    const broken = list.findIndex((x) => parseSteps(x.text, state.catalog).error);
    if (broken >= 0) {
      setCur(broken);
      return toast.error(`“${list[broken].name}” has a mistake`, { description: parseSteps(list[broken].text, state.catalog).error ?? "" });
    }
    setBusy(true);
    try {
      const saved = await api.s75SetMacros(list);
      setState({ ...state, saved });
      setList(saved.macros);
      toast.success("Macros saved on the keyboard");
    } catch (e) {
      toast.error("Can't save the macros", { description: friendly(e) });
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="grid gap-4">
      <S75Untested />
      <div className="grid gap-4 md:grid-cols-[220px_1fr]">
        <Card className="self-start">
          <CardHeader>
            <CardTitle className="text-base">Macros</CardTitle>
            <CardDescription>Assign them to keys on the Keys page.</CardDescription>
          </CardHeader>
          <CardContent className="grid gap-1">
            {list.map((x, i) => (
              <Button key={i} variant={i === cur ? "secondary" : "ghost"} className="justify-start" onClick={() => setCur(i)}>
                <span className="text-muted-foreground w-5 text-xs">{i + 1}</span> {x.name}
              </Button>
            ))}
            <Button
              variant="outline"
              size="sm"
              disabled={list.length >= 100}
              onClick={() => {
                setList([...list, { name: `Macro ${list.length + 1}`, steps: [], text: "" }]);
                setCur(list.length);
              }}
            >
              <Plus /> New macro
            </Button>
          </CardContent>
        </Card>

        {m ? (
          <Card>
            <CardContent className="grid gap-4 pt-6">
              <div className="flex items-end gap-2">
                <div className="grid flex-1 gap-1.5">
                  <Label htmlFor="macro-name">Name</Label>
                  <Input id="macro-name" value={m.name} maxLength={40} onChange={(e) => edit({ name: e.target.value })} />
                </div>
                <Button
                  variant="ghost"
                  title="Delete this macro (keys using later macros move up one)"
                  onClick={() => {
                    setList(list.filter((_, i) => i !== cur));
                    setCur(Math.max(0, cur - 1));
                  }}
                >
                  <Trash2 />
                </Button>
              </div>
              <div className="grid gap-1.5">
                <Label htmlFor="macro-steps">Steps, one per line</Label>
                <Textarea
                  id="macro-steps"
                  rows={8}
                  className={cn("font-mono text-sm", parsed.error && "border-destructive")}
                  value={m.text}
                  placeholder={"ctrl+c\nwait 100\ntext Hello\nclick left"}
                  onChange={(e) => edit({ text: e.target.value })}
                />
                <span className={cn("text-xs", parsed.error ? "text-destructive" : "text-muted-foreground")}>
                  {parsed.error ??
                    `${parsed.steps.length} steps. Write a key or combo (ctrl+c), down/up shift, wait 100, click left, or text Hello.`}
                </span>
              </div>
            </CardContent>
          </Card>
        ) : (
          <Card>
            <CardContent className="text-muted-foreground pt-6 text-sm">No macros yet. Make one with New macro.</CardContent>
          </Card>
        )}
      </div>

      <div className={cn("flex items-center gap-3 rounded-lg border px-4 py-3", dirty && "border-primary/40 sticky bottom-0 bg-background/95 shadow-lg backdrop-blur")}>
        <span className="text-sm">{dirty ? "Changes not on the keyboard yet." : "The keyboard stores all macros together."}</span>
        <Button className="ml-auto" disabled={busy || !dirty} onClick={save}>
          <Save /> Save to keyboard
        </Button>
      </div>
    </div>
  );
}
