import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { toast } from "sonner";
import { Headphones, Info, Mic, MicOff, PhoneCall, RefreshCw, Volume2, VolumeX } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { ColourInput } from "@/components/colour-input";
import { Keyboard } from "@/components/keyboard";
import { cn } from "@/lib/utils";
import {
  api,
  Catalog,
  DiscordEvent,
  DiscordIndicator,
  DiscordSetup,
  DiscordState,
  EffectStyle,
  fromHex,
  Rgb,
  toHex,
} from "@/lib/api";
import { friendly } from "@/lib/errors";

const INDICATORS: { id: DiscordIndicator; label: string; help: string }[] = [
  { id: "speaking", label: "You're talking", help: "While your mic picks you up" },
  { id: "others_speaking", label: "Someone's talking", help: "Anyone else in your voice channel" },
  { id: "muted", label: "Muted", help: "Your mic is muted" },
  { id: "deafened", label: "Deafened", help: "You can't hear the channel (paints over Muted)" },
  { id: "in_voice", label: "In a voice channel", help: "While you're connected to voice" },
];

const EFFECTS: { id: DiscordEvent; label: string }[] = [
  { id: "notification", label: "Notification (DM or mention)" },
  { id: "joined_voice", label: "You join a voice channel" },
  { id: "left_voice", label: "You leave it" },
  { id: "user_joined", label: "Someone joins your channel" },
  { id: "user_left", label: "Someone leaves it" },
];

const STYLES: { id: EffectStyle; label: string }[] = [
  { id: "flash", label: "Flash" },
  { id: "pulse", label: "Pulse" },
  { id: "wave", label: "Wave" },
];

const ROWS: Record<string, string[]> = {
  "F1-F12": ["f1", "f2", "f3", "f4", "f5", "f6", "f7", "f8", "f9", "f10", "f11", "f12"],
  "Number row": ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"],
  Space: ["space"],
  Esc: ["esc"],
};

export function DiscordPage({ catalog }: { catalog: Catalog }) {
  const [state, setState] = useState<DiscordState | null>(null);
  const [frame, setFrame] = useState<[number, Rgb][] | null>(null);
  const [busy, setBusy] = useState(false);
  const [picking, setPicking] = useState<DiscordIndicator | null>(null);
  const [clientId, setClientId] = useState("");
  const [secret, setSecret] = useState("");
  const pending = useRef<number | null>(null);

  useEffect(() => {
    api.discordGet().then((s) => {
      setState(s);
      setClientId(s.client_id);
    });
    const offs = [
      listen<DiscordState>("discord-state", (e) => setState((s) => (s ? { ...e.payload, setup: s.setup } : e.payload))),
      listen<[number, Rgb][]>("discord-frame", (e) => setFrame(e.payload)),
    ];
    return () => offs.forEach((p) => p.then((off) => off()));
  }, []);

  if (!state) return null;
  const s = state.setup;
  const byName = (names: string[]) => names.map((n) => catalog.keys.find((k) => k.name === n)?.index).filter((i): i is number => i !== undefined);

  /** Change the setup: shown at once, saved and applied shortly after. */
  const set = (next: DiscordSetup) => {
    setState({ ...state, setup: next });
    if (pending.current) window.clearTimeout(pending.current);
    pending.current = window.setTimeout(() => api.discordSet({ setup: next }).catch(() => {}), 150);
  };
  const setIndicator = (id: DiscordIndicator, p: Partial<DiscordSetup["indicators"][number]>) =>
    set({ ...s, indicators: s.indicators.map((i) => (i.indicator === id ? { ...i, ...p } : i)) });
  const setEffect = (id: DiscordEvent, p: Partial<DiscordSetup["effects"][number]>) =>
    set({ ...s, effects: s.effects.map((e) => (e.event === id ? { ...e, ...p } : e)) });

  const run = async (what: string, f: () => Promise<DiscordState>) => {
    setBusy(true);
    try {
      setState(await f());
    } catch (e) {
      toast.error(what, { description: friendly(e) });
    } finally {
      setBusy(false);
    }
  };

  const saveApp = () =>
    run("Can't save the Discord app", async () => {
      const r = await api.discordSet({ clientId, ...(secret ? { clientSecret: secret } : {}) });
      setSecret("");
      toast.success("Saved", { description: r.on ? "Connecting to Discord." : "Turn Discord lighting on to connect." });
      return r;
    });

  const picked = picking ? (s.indicators.find((i) => i.indicator === picking)?.keys ?? []) : [];
  const pickKey = (index: number) => {
    if (!picking) return;
    setIndicator(picking, { keys: picked.includes(index) ? picked.filter((k) => k !== index) : [...picked, index] });
  };

  const colours = new Map<number, Rgb>(frame ?? []);
  const v = state.voice;

  return (
    <div className="grid gap-4">
      <Card>
        <CardHeader className="flex flex-row items-start justify-between gap-4">
          <div className="grid gap-1.5">
            <CardTitle className="flex items-center gap-2">
              <Headphones className="size-5" /> Discord lighting
            </CardTitle>
            <CardDescription>{state.status || "Starting."}</CardDescription>
            {state.on && (
              <div className="flex flex-wrap gap-1.5 pt-1">
                <Badge variant={v.in_voice ? "secondary" : "outline"} className="gap-1">
                  <PhoneCall className="size-3" /> {v.in_voice ? "In voice" : "Not in voice"}
                </Badge>
                {v.in_voice && (
                  <>
                    <Badge variant="outline" className="gap-1">
                      {v.muted ? <MicOff className="size-3" /> : <Mic className="size-3" />} {v.muted ? "Muted" : "Mic on"}
                    </Badge>
                    <Badge variant="outline" className="gap-1">
                      {v.deafened ? <VolumeX className="size-3" /> : <Volume2 className="size-3" />} {v.deafened ? "Deafened" : "Hearing"}
                    </Badge>
                  </>
                )}
              </div>
            )}
          </div>
          <div className="flex items-center gap-2 text-sm">
            <span className={cn(!state.on && "text-foreground font-medium")}>Off</span>
            <Switch
              checked={state.on}
              disabled={busy}
              onCheckedChange={(on) => run("Can't change Discord lighting", () => api.discordSet({ on }))}
              aria-label="Discord lighting"
            />
            <span className={cn(state.on && "text-foreground font-medium")}>On</span>
          </div>
        </CardHeader>
        <CardContent className="grid gap-2">
          {picking && <p className="text-sm">Click keys to add or take them out. Press Done when finished.</p>}
          <Keyboard
            keys={catalog.keys}
            selected={new Set(picked)}
            onKey={picking ? pickKey : undefined}
            look={(i) => {
              if (picking) return {};
              const c = colours.get(i);
              return c ? { fill: toHex(c) } : {};
            }}
          />
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">Your Discord app</CardTitle>
          <CardDescription>
            Discord only lets apps it knows read voice and notifications. So Lumen75 uses an app you make yourself, once:
          </CardDescription>
        </CardHeader>
        <CardContent className="grid gap-4">
          <ol className="text-muted-foreground list-decimal space-y-1 pl-5 text-sm">
            <li>
              Go to <span className="text-foreground font-medium select-all">discord.com/developers/applications</span> and
              press <span className="text-foreground">New Application</span>. Any name works, Lumen75 for example.
            </li>
            <li>
              Open <span className="text-foreground">OAuth2</span>. Copy the <span className="text-foreground">Client ID</span>, then
              press <span className="text-foreground">Reset Secret</span> and copy the secret.
            </li>
            <li>
              Under <span className="text-foreground">Redirects</span>, add <span className="text-foreground select-all">http://localhost</span> and
              save.
            </li>
            <li>Paste both below, save, and turn Discord lighting on. The Discord app asks you to approve Lumen75 once.</li>
          </ol>
          <div className="grid gap-3 md:grid-cols-2">
            <div className="grid gap-1.5">
              <Label htmlFor="discord-id">Client ID</Label>
              <Input id="discord-id" value={clientId} onChange={(e) => setClientId(e.target.value)} placeholder="123456789012345678" />
            </div>
            <div className="grid gap-1.5">
              <Label htmlFor="discord-secret">Client Secret</Label>
              <Input
                id="discord-secret"
                type="password"
                value={secret}
                onChange={(e) => setSecret(e.target.value)}
                placeholder={state.has_secret ? "Saved (type to replace it)" : "Paste the secret"}
              />
            </div>
          </div>
          <div className="flex flex-wrap items-center gap-2">
            <Button onClick={saveApp} disabled={busy || !clientId.trim() || (!secret && !state.has_secret)}>
              Save
            </Button>
            <Button variant="outline" onClick={() => run("Can't reconnect", () => api.discordReconnect(false))} disabled={busy || !state.on}>
              <RefreshCw /> Reconnect
            </Button>
            {state.signed_in && (
              <Button variant="ghost" onClick={() => run("Can't sign out", () => api.discordReconnect(true))} disabled={busy}>
                Sign out
              </Button>
            )}
          </div>
          <p className="text-muted-foreground text-xs">
            The ID and secret are saved on this PC, in Lumen75's settings. Lumen75 only asks Discord to read your voice state
            and notifications.
          </p>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">While it's true</CardTitle>
          <CardDescription>These keys stay lit. Lower ones paint over higher ones.</CardDescription>
        </CardHeader>
        <CardContent className="grid gap-3">
          {INDICATORS.map((meta) => {
            const ind = s.indicators.find((i) => i.indicator === meta.id);
            if (!ind) return null;
            return (
              <div key={meta.id} className="grid items-center gap-3 border-b pb-3 last:border-0 last:pb-0 md:grid-cols-[1fr_auto]">
                <div className="flex items-center gap-3">
                  <Switch checked={ind.on} onCheckedChange={(on) => setIndicator(meta.id, { on })} aria-label={meta.label} />
                  <div>
                    <div className="text-sm font-medium">{meta.label}</div>
                    <div className="text-muted-foreground text-xs">{meta.help}</div>
                  </div>
                </div>
                <div className={cn("flex flex-wrap items-center gap-1.5", !ind.on && "opacity-50")}>
                  <ColourInput label={meta.label} value={toHex(ind.color)} onChange={(h) => setIndicator(meta.id, { color: fromHex(h) })} />
                  {Object.entries(ROWS).map(([name, names]) => (
                    <Button key={name} size="sm" variant="outline" onClick={() => setIndicator(meta.id, { keys: byName(names) })}>
                      {name}
                    </Button>
                  ))}
                  <Button size="sm" variant={picking === meta.id ? "secondary" : "outline"} onClick={() => setPicking(picking === meta.id ? null : meta.id)}>
                    {picking === meta.id ? "Done" : "Pick keys"}
                  </Button>
                  <span className="text-muted-foreground text-xs">{ind.keys.length} keys</span>
                </div>
              </div>
            );
          })}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">Moments</CardTitle>
          <CardDescription>A short effect over the whole board.</CardDescription>
        </CardHeader>
        <CardContent className="grid gap-3">
          {EFFECTS.map((meta) => {
            const fx = s.effects.find((e) => e.event === meta.id);
            if (!fx) return null;
            return (
              <div key={meta.id} className="flex flex-wrap items-center gap-3">
                <Switch checked={fx.on} onCheckedChange={(on) => setEffect(meta.id, { on })} aria-label={meta.label} />
                <span className="min-w-56 text-sm">{meta.label}</span>
                <div className={cn("flex flex-wrap items-center gap-2", !fx.on && "opacity-50")}>
                  <Select value={fx.style} onValueChange={(style) => setEffect(meta.id, { style: style as EffectStyle })}>
                    <SelectTrigger className="w-28">
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
                  <ColourInput label={meta.label} value={toHex(fx.color)} onChange={(h) => setEffect(meta.id, { color: fromHex(h) })} />
                </div>
              </div>
            );
          })}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">Behind it</CardTitle>
          <CardDescription>
            Over an animation, audio or League lighting, Discord paints on top of it. With nothing else running, Discord
            lights the keyboard on its own while it has something to show, with this behind it.
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-wrap items-center gap-3">
          <ToggleGroup
            type="single"
            variant="outline"
            value={s.background.kind}
            onValueChange={(k) => k && set({ ...s, background: { ...s.background, kind: k as "off" | "solid" } })}
          >
            <ToggleGroupItem value="off">Dark</ToggleGroupItem>
            <ToggleGroupItem value="solid">A colour</ToggleGroupItem>
          </ToggleGroup>
          {s.background.kind === "solid" && (
            <ColourInput label="Background" value={toHex(s.background.color)} onChange={(h) => set({ ...s, background: { ...s.background, color: fromHex(h) } })} />
          )}
        </CardContent>
      </Card>

      <p className="text-muted-foreground flex items-start gap-2 text-xs">
        <Info className="mt-0.5 size-3.5 shrink-0" />
        Uses Discord's local connection on this PC (RPC), so the Discord desktop app has to run. When Discord has nothing
        to show and nothing else streams, the keyboard keeps its own effect.
      </p>
    </div>
  );
}
