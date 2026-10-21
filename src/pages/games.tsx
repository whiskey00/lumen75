import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { toast } from "sonner";
import { ArrowLeft, ArrowRight, Info, Plus, Swords, X } from "lucide-react";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { Keyboard } from "@/components/keyboard";
import { PresetBar } from "@/components/preset-bar";
import { ColourInput } from "@/components/colour-input";
import { cn } from "@/lib/utils";
import {
  api,
  Catalog,
  EffectStyle,
  fromHex,
  GameEventKind,
  LeagueBar,
  LeagueSetup,
  LeagueState,
  LeagueValue,
  Presets,
  Rgb,
  toHex,
} from "@/lib/api";

const VALUES: { id: LeagueValue; label: string }[] = [
  { id: "health", label: "Health" },
  { id: "resource", label: "Mana / energy" },
  { id: "gold", label: "Gold" },
  { id: "level", label: "Level" },
  { id: "respawn", label: "Respawn countdown" },
];

const EVENTS: Record<GameEventKind, string> = {
  kill: "Your kill",
  death: "Your death",
  assist: "Your assist",
  firstblood: "First blood (yours)",
  multikill: "Multi-kill (stronger for more)",
  ace: "Ace (your team)",
  dragon: "Dragon (your team)",
  herald: "Herald (your team)",
  baron: "Baron (your team)",
  turret: "Turret taken",
  inhibitor: "Inhibitor taken",
  levelup: "Level up",
  win: "Win",
  lose: "Lose",
};

const STYLES: { id: EffectStyle; label: string }[] = [
  { id: "flash", label: "Flash" },
  { id: "pulse", label: "Pulse" },
  { id: "wave", label: "Wave" },
];

const NUMBER_ROW = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"];
const F_ROW = ["f1", "f2", "f3", "f4", "f5", "f6", "f7", "f8", "f9", "f10", "f11", "f12"];

/** Which key list is being picked on the keyboard: a bar's, or the dead state's countdown. */
type Picking = { bar: number } | "countdown" | null;

export function GamesPage({ catalog }: { catalog: Catalog }) {
  const [state, setState] = useState<LeagueState | null>(null);
  const [frame, setFrame] = useState<[number, Rgb][] | null>(null);
  const [busy, setBusy] = useState(false);
  const [picking, setPicking] = useState<Picking>(null);
  const [mine, setMine] = useState<Presets["league"]>([]);
  const [ready, setReady] = useState<Presets["league_ready"]>([]);
  const pending = useRef<number | null>(null);

  useEffect(() => {
    api.leagueGet().then(setState);
    api
      .presetsGet()
      .then((p) => {
        setMine(p.league);
        setReady(p.league_ready);
      })
      .catch(() => {});
    const offs = [
      listen<[number, Rgb][]>("league-frame", (e) => setFrame(e.payload)),
      listen<LeagueState>("league-state", (e) => {
        setState((s) => (s ? { ...e.payload, setup: s.setup } : e.payload));
        if (!e.payload.live) setFrame(null);
      }),
      listen<string>("league-error", (e) => toast.error("League lighting", { description: e.payload })),
    ];
    return () => offs.forEach((p) => p.then((off) => off()));
  }, []);

  if (!state) return null;
  const s = state.setup;
  const byName = (names: string[]) => names.map((n) => catalog.keys.find((k) => k.name === n)?.index).filter((i): i is number => i !== undefined);

  /** Change the setup: shown at once, saved and applied shortly after. */
  const set = (next: LeagueSetup) => {
    setState({ ...state, setup: next });
    if (pending.current) window.clearTimeout(pending.current);
    pending.current = window.setTimeout(() => api.leagueSet({ setup: next }).catch(() => {}), 120);
  };
  const setBar = (i: number, p: Partial<LeagueBar>) => set({ ...s, bars: s.bars.map((b, j) => (j === i ? { ...b, ...p } : b)) });

  const toggle = async (on: boolean) => {
    setBusy(true);
    try {
      setState({ ...(await api.leagueSet({ on })), setup: s });
    } catch (e) {
      toast.error("Can't change League lighting", { description: String(e) });
    } finally {
      setBusy(false);
    }
  };

  const pickedKeys = picking === null ? [] : picking === "countdown" ? s.dead.countdown_keys : (s.bars[picking.bar]?.keys ?? []);
  /** Clicking a key adds it to the end of the list being picked, or takes it out. */
  const pickKey = (index: number) => {
    const keys = pickedKeys.includes(index) ? pickedKeys.filter((k) => k !== index) : [...pickedKeys, index];
    if (picking === "countdown") set({ ...s, dead: { ...s.dead, countdown_keys: keys } });
    else if (picking) setBar(picking.bar, { keys });
  };

  const colours = new Map<number, Rgb>(frame ?? []);
  const status = state.status ?? (!state.on ? "Off." : state.live ? "Lighting your match." : "Waiting for a match.");

  const keysPicker = (keys: number[], onKeys: (k: number[]) => void, target: Picking) => (
    <div className="flex flex-wrap items-center gap-1.5">
      <Button size="sm" variant="outline" onClick={() => onKeys(byName(NUMBER_ROW))}>
        Number row
      </Button>
      <Button size="sm" variant="outline" onClick={() => onKeys(byName(F_ROW))}>
        F1-F12
      </Button>
      <Button
        size="sm"
        variant={JSON.stringify(picking) === JSON.stringify(target) ? "secondary" : "outline"}
        onClick={() => setPicking(JSON.stringify(picking) === JSON.stringify(target) ? null : target)}
      >
        {JSON.stringify(picking) === JSON.stringify(target) ? "Done picking" : "Pick keys"}
      </Button>
      <span className="text-muted-foreground text-xs">{keys.length} keys</span>
    </div>
  );

  return (
    <div className="grid gap-4">
      <Card>
        <CardHeader className="flex flex-row items-start justify-between gap-4">
          <div className="grid gap-1.5">
            <CardTitle className="flex items-center gap-2">
              <Swords className="size-5" /> League of Legends lighting
            </CardTitle>
            <CardDescription>{status}</CardDescription>
          </div>
          <div className="flex items-center gap-2 text-sm">
            <span className={cn(!state.on && "text-foreground font-medium")}>Off</span>
            <Switch checked={state.on} disabled={busy} onCheckedChange={toggle} aria-label="League of Legends lighting" />
            <span className={cn(state.on && "text-foreground font-medium")}>On</span>
          </div>
        </CardHeader>
        <CardContent className="grid gap-2">
          {picking !== null && <p className="text-sm">Click keys in the order the bar should fill. Click a key again to take it out.</p>}
          <Keyboard
            keys={catalog.keys}
            selected={new Set(pickedKeys)}
            onKey={picking !== null ? (i) => pickKey(i) : undefined}
            look={(i) => {
              if (picking !== null) {
                const at = pickedKeys.indexOf(i);
                return at >= 0 ? { sub: String(at + 1) } : {};
              }
              const c = colours.get(i);
              return c ? { fill: toHex(c) } : {};
            }}
          />
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">Background</CardTitle>
          <CardDescription>For keys no bar uses.</CardDescription>
        </CardHeader>
        <CardContent className="flex flex-wrap items-center gap-3">
          <ToggleGroup
            type="single"
            variant="outline"
            value={s.background.kind}
            onValueChange={(v) => v && set({ ...s, background: { ...s.background, kind: v as LeagueSetup["background"]["kind"] } })}
          >
            <ToggleGroupItem value="team">Team colour</ToggleGroupItem>
            <ToggleGroupItem value="solid">One colour</ToggleGroupItem>
            <ToggleGroupItem value="off">Off</ToggleGroupItem>
          </ToggleGroup>
          {s.background.kind === "solid" && (
            <ColourInput
              value={toHex(s.background.color)}
              onChange={(hex) => set({ ...s, background: { ...s.background, color: fromHex(hex) } })}
              label="Background colour"
            />
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">Bars</CardTitle>
          <CardDescription>Values that go up and down, shown across keys.</CardDescription>
        </CardHeader>
        <CardContent className="grid gap-4">
          {s.bars.map((b, i) => (
            <div key={i} className="grid gap-3 rounded-lg border p-3">
              <div className="flex flex-wrap items-center gap-3">
                <Select value={b.value} onValueChange={(v) => setBar(i, { value: v as LeagueValue })}>
                  <SelectTrigger className="w-48">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {VALUES.map((v) => (
                      <SelectItem key={v.id} value={v.id}>
                        {v.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
                <ToggleGroup
                  type="single"
                  variant="outline"
                  value={b.reverse ? "left" : "right"}
                  onValueChange={(v) => v && setBar(i, { reverse: v === "left" })}
                >
                  <ToggleGroupItem value="right" aria-label="Fills left to right">
                    <ArrowRight />
                  </ToggleGroupItem>
                  <ToggleGroupItem value="left" aria-label="Fills right to left">
                    <ArrowLeft />
                  </ToggleGroupItem>
                </ToggleGroup>
                {b.value === "gold" && (
                  <div className="flex items-center gap-2">
                    <Label htmlFor={`max-${i}`}>Full at</Label>
                    <Input
                      id={`max-${i}`}
                      type="number"
                      min={100}
                      step={100}
                      value={b.max}
                      onChange={(e) => setBar(i, { max: Math.max(1, Number(e.target.value) || 1) })}
                      className="w-24"
                    />
                    <span className="text-muted-foreground text-sm">gold</span>
                  </div>
                )}
                <Button
                  size="icon"
                  variant="ghost"
                  className="ml-auto"
                  aria-label="Remove this bar"
                  onClick={() => {
                    setPicking(null);
                    set({ ...s, bars: s.bars.filter((_, j) => j !== i) });
                  }}
                >
                  <X />
                </Button>
              </div>
              {keysPicker(b.keys, (keys) => setBar(i, { keys }), { bar: i })}
              <div className="flex flex-wrap items-center gap-4">
                <div className="grid gap-1">
                  <Label className="text-xs">Full</Label>
                  <ColourInput value={toHex(b.full)} onChange={(hex) => setBar(i, { full: fromHex(hex) })} label="Full colour" />
                </div>
                <div className="grid gap-1">
                  <Label className="text-xs">Empty</Label>
                  <ColourInput value={toHex(b.empty)} onChange={(hex) => setBar(i, { empty: fromHex(hex) })} label="Empty colour" />
                </div>
                {b.value === "resource" && (
                  <span className="text-muted-foreground text-xs">Black as the full colour uses the resource's own colour.</span>
                )}
              </div>
            </div>
          ))}
          <Button
            variant="outline"
            className="justify-self-start"
            onClick={() => set({ ...s, bars: [...s.bars, { value: "health", keys: [], full: [0, 255, 0], empty: [255, 0, 0], reverse: false, max: 3000 }] })}
          >
            <Plus /> Add a bar
          </Button>
        </CardContent>
      </Card>

      <Card>
        <CardHeader className="flex flex-row items-start justify-between gap-4">
          <div className="grid gap-1.5">
            <CardTitle className="text-base">While you're dead</CardTitle>
            <CardDescription>Everything dims, and the respawn countdown runs as a bar.</CardDescription>
          </div>
          <Switch checked={s.dead.on} onCheckedChange={(on) => set({ ...s, dead: { ...s.dead, on } })} aria-label="Dead state" />
        </CardHeader>
        {s.dead.on && (
          <CardContent className="grid gap-3">
            {keysPicker(s.dead.countdown_keys, (countdown_keys) => set({ ...s, dead: { ...s.dead, countdown_keys } }), "countdown")}
            <div className="flex flex-wrap items-center gap-4">
              <div className="grid gap-1">
                <Label className="text-xs">Dimmed keys</Label>
                <ColourInput value={toHex(s.dead.dim)} onChange={(hex) => set({ ...s, dead: { ...s.dead, dim: fromHex(hex) } })} label="Dim colour" />
              </div>
              <div className="grid gap-1">
                <Label className="text-xs">Countdown</Label>
                <ColourInput
                  value={toHex(s.dead.countdown)}
                  onChange={(hex) => set({ ...s, dead: { ...s.dead, countdown: fromHex(hex) } })}
                  label="Countdown colour"
                />
              </div>
            </div>
          </CardContent>
        )}
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">Effects</CardTitle>
          <CardDescription>Short effects for moments in the match, over everything else.</CardDescription>
        </CardHeader>
        <CardContent className="grid gap-2">
          {s.events.map((e, i) => {
            const setEffect = (p: Partial<typeof e>) => set({ ...s, events: s.events.map((x, j) => (j === i ? { ...x, ...p } : x)) });
            return (
              <div key={e.event} className={cn("flex flex-wrap items-center gap-3", !e.on && "opacity-60")}>
                <Switch checked={e.on} onCheckedChange={(on) => setEffect({ on })} aria-label={EVENTS[e.event]} />
                <span className="w-56 text-sm">{EVENTS[e.event]}</span>
                <Select value={e.style} onValueChange={(v) => setEffect({ style: v as EffectStyle })} disabled={!e.on}>
                  <SelectTrigger size="sm" className="w-28">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {STYLES.map((st) => (
                      <SelectItem key={st.id} value={st.id}>
                        {st.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
                {e.event === "dragon" && (
                  <Button
                    size="sm"
                    variant={e.color === null ? "secondary" : "outline"}
                    disabled={!e.on}
                    onClick={() => setEffect({ color: e.color === null ? [255, 255, 255] : null })}
                  >
                    Drake's colour
                  </Button>
                )}
                {e.color !== null && (
                  <ColourInput value={toHex(e.color)} onChange={(hex) => setEffect({ color: fromHex(hex) })} label={`${EVENTS[e.event]} colour`} />
                )}
                <div className="flex items-center gap-1.5">
                  <Input
                    type="number"
                    min={0.2}
                    max={10}
                    step={0.1}
                    value={e.ms / 1000}
                    disabled={!e.on}
                    onChange={(ev) => setEffect({ ms: Math.round(Math.min(10, Math.max(0.2, Number(ev.target.value) || 0.2)) * 1000) })}
                    className="w-20"
                    aria-label={`${EVENTS[e.event]} duration`}
                  />
                  <span className="text-muted-foreground text-sm">s</span>
                </div>
              </div>
            );
          })}
        </CardContent>
      </Card>

      <Card>
        <CardContent className="grid gap-4 pt-6">
          <div className="grid gap-2">
            <Label>Ready-made</Label>
            <div className="flex flex-wrap gap-2">
              {ready.map((p) => (
                <Button
                  key={p.name}
                  variant="outline"
                  onClick={() => {
                    setPicking(null);
                    set(p.setup);
                    toast.success(`Loaded “${p.name}”`, { description: "Change anything, then save it as your own." });
                  }}
                >
                  <span className="flex -space-x-1" aria-hidden>
                    {[p.setup.background.color, ...p.setup.bars.map((b) => b.full)]
                      .filter((c) => c.some((v) => v > 0))
                      .map((c, i) => (
                        <span key={i} className="border-background size-3 rounded-full border" style={{ background: toHex(c) }} />
                      ))}
                  </span>
                  {p.name}
                </Button>
              ))}
            </div>
          </div>
          <PresetBar
            title="My League setups"
            saves="this League setup: background, bars, dead state and effects"
            items={mine.map((p) => ({ name: p.name, swatch: p.setup.bars.map((b) => toHex(b.full)) }))}
            onLoad={(name) => {
              const p = mine.find((x) => x.name === name);
              if (p) {
                setPicking(null);
                set(p.setup);
              }
            }}
            onSave={async (name) => {
              try {
                setMine((await api.leaguePresetSave(name, s)).league);
                toast.success(`Saved “${name.trim()}”`);
                return true;
              } catch (e) {
                toast.error("Can't save the setup", { description: String(e) });
                return false;
              }
            }}
            onDelete={async (name) => {
              setMine((await api.presetDelete("league", name)).league);
            }}
          />
        </CardContent>
      </Card>

      <p className="text-muted-foreground flex items-start gap-2 text-xs">
        <Info className="mt-0.5 size-3.5 shrink-0" />
        Uses only the match data League itself shares on this PC, so no cooldowns. It works over the USB cable and the
        2.4G dongle, takes over when a match starts, and brings back your animation or audio lighting when it ends.
        Start one during a match and League lighting steps aside until the next.
      </p>
      {/* Riot's developer policy asks for this wording where players see it */}
      <p className="text-muted-foreground text-xs leading-relaxed">
        Lumen75 isn't endorsed by Riot Games and doesn't reflect the views or opinions of Riot Games or anyone officially
        involved in producing or managing Riot Games properties. Riot Games, and all associated properties are
        trademarks or registered trademarks of Riot Games, Inc.
      </p>
    </div>
  );
}
