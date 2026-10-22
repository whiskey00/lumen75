import { useCallback, useEffect, useRef, useState } from "react";
import { toast } from "sonner";
import { listen } from "@tauri-apps/api/event";
import { AudioLines, BatteryFull, Gamepad2, Headphones, Monitor, Sparkles, BatteryLow, BatteryMedium, Grid3x3, Keyboard as KeyboardIcon, Loader2, RefreshCw, Settings2, SquareFunction, Square, Sun, TriangleAlert } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { cn } from "@/lib/utils";
import { api, Board, Catalog, Connection, merge, Playing, ReadPart, ReadProgress, Snapshot, Status } from "@/lib/api";
import { LightingPage } from "@/pages/lighting";
import { PerKeyPage } from "@/pages/perkey";
import { KeysPage } from "@/pages/keys";
import { MacrosPage } from "@/pages/macros";
import { DevicePage } from "@/pages/device";
import { AudioPage } from "@/pages/audio";
import { AnimationsPage } from "@/pages/animations";
import { GamesPage } from "@/pages/games";
import { DiscordPage } from "@/pages/discord";
import { S75LightingPage } from "@/pages/s75-lighting";
import { S75DevicePage } from "@/pages/s75-device";
import { S75PerKeyPage } from "@/pages/s75-perkey";
import { S75KeysPage } from "@/pages/s75-keys";
import { S75MacrosPage } from "@/pages/s75-macros";
import { S75ScreenPage } from "@/pages/s75-screen";
import { friendly } from "@/lib/errors";

type Page = "lighting" | "perkey" | "animations" | "audio" | "games" | "discord" | "keys" | "macros" | "screen" | "device";

/**
 * `board`: the page shows what was read from the keyboard, so it waits for the read. The others
 * (streamed lighting) work at once. `wireless`: not over the USB cable yet.
 */
/** Pages an S75 Pro has (its own versions); the others need the F75. */
const S75_PAGES: Page[] = ["lighting", "perkey", "keys", "macros", "screen", "device"];

const PAGES: { id: Page; label: string; icon: typeof Sun; wireless?: boolean; board?: boolean; s75only?: boolean }[] = [
  { id: "lighting", label: "Lighting", icon: Sun, board: true },
  { id: "perkey", label: "Per-key", icon: Grid3x3, wireless: true, board: true },
  { id: "animations", label: "Animations", icon: Sparkles },
  { id: "audio", label: "Audio", icon: AudioLines },
  { id: "games", label: "Games", icon: Gamepad2 },
  { id: "discord", label: "Discord", icon: Headphones },
  // over the cable the macros aren't read (the page needs them) and key writes don't work yet
  { id: "keys", label: "Keys", icon: KeyboardIcon, wireless: true, board: true },
  { id: "macros", label: "Macros", icon: SquareFunction, wireless: true, board: true },
  { id: "screen", label: "Screen", icon: Monitor, s75only: true },
  { id: "device", label: "Device", icon: Settings2, board: true },
];

const CABLE = "USB cable";

/** Everything the app reads from the keyboard, and what each page needs of it. */
const ALL_PARTS: ReadPart[] = ["lighting", "keymap", "battery", "custom", "macros"];
const PAGE_PARTS: Partial<Record<Page, ReadPart[]>> = {
  lighting: ["lighting"],
  perkey: ["custom", "lighting"],
  keys: ["keymap", "macros"],
  macros: ["macros", "keymap"],
  device: ["lighting"],
};

/**
 * What was last read from the keyboard, kept in this app. The keyboard stores the same
 * settings whatever the connection, so over Bluetooth (where reading everything takes about
 * 2 minutes) the app opens with this and reads only what the page in front needs.
 */
const REMEMBERED = "lumen75.board";
function remembered(): Board | null {
  try {
    const b = JSON.parse(localStorage.getItem(REMEMBERED) ?? "null");
    return b && b.effects && b.keymap ? { ...b, battery: null } : null;
  } catch {
    return null;
  }
}
const WELCOMED = "lumen75.welcomed";

/** The connection choices, with the device mode each one finds (none: any). */
const CONNECTIONS: { id: Connection; label: string; mode?: string }[] = [
  { id: "auto", label: "Auto" },
  { id: "cable", label: "USB cable", mode: CABLE },
  { id: "dongle", label: "2.4G dongle", mode: "2.4G dongle" },
  { id: "bluetooth", label: "Bluetooth", mode: "Bluetooth" },
];

export default function App() {
  const [catalog, setCatalog] = useState<Catalog | null>(null);
  const [board, setBoard] = useState<Board | null>(null);
  const [status, setStatus] = useState<Status | null>(null);
  const [page, setPage] = useState<Page>("lighting");
  const [busy, setBusy] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [connection, setConnection] = useState<Connection>("auto");
  const [progress, setProgress] = useState<ReadProgress | null>(null);
  const [playing, setPlaying] = useState<Playing | null>(null);
  // the first start shows a short welcome (connection, official app, backup)
  const [welcome, setWelcome] = useState(() => {
    try {
      return localStorage.getItem(WELCOMED) === null;
    } catch {
      return false;
    }
  });
  const closeWelcome = () => {
    setWelcome(false);
    try {
      localStorage.setItem(WELCOMED, "1");
    } catch {
      /* storage unavailable: it shows again next time */
    }
  };
  const busyRef = useRef(false);

  const refreshStatus = useCallback(() => api.status().then(setStatus).catch(() => {}), []);

  // tables read since the app started (the rest of `board` may come from memory)
  const fresh = useRef<Set<ReadPart>>(new Set());
  const [bluetooth, setBluetooth] = useState(false);
  const [updating, setUpdating] = useState(false);

  /** Read everything (start-up over 2.4G and the cable, and Refresh). */
  const reload = useCallback(async () => {
    setLoading(true);
    setError(null);
    setProgress(null);
    try {
      const s = await api.readAll();
      setBoard(merge(null, s));
      fresh.current = new Set(ALL_PARTS);
      setBluetooth(s.mode === "Bluetooth");
    } catch (e) {
      setError(friendly(e));
    } finally {
      setLoading(false);
      refreshStatus();
    }
  }, [refreshStatus]);

  /** Read some tables into what's shown, in the background. */
  const readParts = useCallback(async (parts: ReadPart[]) => {
    setUpdating(true);
    setProgress(null);
    try {
      const s = await api.readParts(parts);
      setBoard((b) => merge(b, s));
      parts.forEach((p) => fresh.current.add(p));
    } catch (e) {
      // not asked again automatically (Refresh reads everything)
      parts.forEach((p) => fresh.current.add(p));
      toast.error("Can't read the keyboard", { description: friendly(e) });
    } finally {
      setUpdating(false);
    }
  }, []);

  /** Start-up: over Bluetooth with something remembered, open with that and read little. */
  const start = useCallback(async () => {
    fresh.current = new Set();
    const memory = remembered();
    if (!memory) return reload();
    let mode: string;
    try {
      mode = await api.connectionMode();
    } catch {
      return reload(); // not reachable: the usual "can't reach" card and Try again
    }
    if (mode !== "Bluetooth") return reload();
    setBluetooth(true);
    setBoard({ ...memory, mode });
  }, [reload]);

  useEffect(() => {
    api.catalog().then(setCatalog);
    api.appSettingsGet().then((s) => setConnection(s.connection)).catch(() => {});
    start();
  }, [start]);

  // keep what was read, for the next start over Bluetooth
  useEffect(() => {
    if (!board) return;
    try {
      localStorage.setItem(REMEMBERED, JSON.stringify({ ...board, battery: null }));
    } catch {
      /* storage unavailable: the next start reads everything */
    }
  }, [board]);

  const chooseConnection = async (c: Connection) => {
    setConnection(c);
    setBoard(null);
    try {
      await api.appSettingsSet({ connection: c });
    } catch (e) {
      setError(friendly(e));
      return;
    }
    start();
  };

  useEffect(() => {
    const off = listen<ReadProgress>("read-progress", (e) => setProgress(e.payload));
    return () => void off.then((f) => f());
  }, []);

  // what's lighting the keyboard (animations, audio, League), for the header
  useEffect(() => {
    const check = () => api.playing().then(setPlaying).catch(() => {});
    check();
    const t = setInterval(check, 1500);
    return () => clearInterval(t);
  }, []);

  const stopPlaying = async () => {
    try {
      setPlaying(await api.stopPlaying());
    } catch (e) {
      toast.error("Can't stop it", { description: friendly(e) });
    }
  };

  // device list and the official-app check; the keyboard itself is only touched by commands
  useEffect(() => {
    const t = setInterval(() => !busyRef.current && refreshStatus(), 5000);
    return () => clearInterval(t);
  }, [refreshStatus]);

  const setBusyBoth = (b: boolean) => {
    busyRef.current = b;
    setBusy(b);
  };

  const run = useCallback(async (label: string, op: () => Promise<Snapshot>) => {
    setBusyBoth(true);
    const id = toast.loading(label);
    try {
      const s = await op();
      setBoard((b) => merge(b, s));
      toast.success(label, { id });
      return true;
    } catch (e) {
      toast.error(label, { id, description: friendly(e), duration: 8000 });
      return false;
    } finally {
      setBusyBoth(false);
    }
  }, []);

  const wanted = CONNECTIONS.find((c) => c.id === connection)?.mode;
  const supported = status?.devices.find((d) => d.supported && (!wanted || d.mode === wanted));
  const onCable = board?.mode === CABLE;
  // a page the cable can't serve falls back to Lighting
  // an S75 Pro with no F75 around: the app shows the S75 Pro's pages
  const s75Mode = !!status?.s75 && !(status?.devices ?? []).some((d) => d.supported);
  const off = (p: Page) => (s75Mode ? !S75_PAGES.includes(p) : onCable && !!PAGES.find((x) => x.id === p)?.wireless);
  const shown: Page = off(page) ? "lighting" : page;
  const waitsForBoard = !s75Mode && !!PAGES.find((p) => p.id === shown)?.board && !board;

  // over Bluetooth, read what this page needs (and the battery) the first time it's opened
  useEffect(() => {
    if (!bluetooth || !board || s75Mode || updating || loading) return;
    const missing = [...(PAGE_PARTS[shown] ?? []), "battery" as ReadPart].filter((p) => !fresh.current.has(p));
    if (missing.length) readParts(missing);
  }, [bluetooth, board, shown, s75Mode, updating, loading, readParts]);
  const PLAYING_PAGE: Record<Playing["kind"], Page> = { animation: "animations", audio: "audio", league: "games" };

  return (
    <div className="bg-background text-foreground flex h-screen">
      <aside className="bg-sidebar flex w-52 shrink-0 flex-col border-r">
        <div className="flex items-center gap-3 px-4 py-5">
          <img src="/logo.svg" alt="" className="size-10 shrink-0" />
          <div>
            <div className="text-lg leading-tight font-semibold tracking-tight">
              Lumen<span className="text-muted-foreground font-normal">75</span>
            </div>
            <div className="text-muted-foreground text-xs">for AULA F75</div>
          </div>
        </div>
        <nav className="grid gap-1 px-3">
          {PAGES.filter((p) => s75Mode || !p.s75only).map((p) => (
            <Button
              key={p.id}
              variant={page === p.id ? "secondary" : "ghost"}
              className="justify-start"
              disabled={off(p.id)}
              title={
                s75Mode && off(p.id)
                  ? "Not on the S75 Pro yet"
                  : onCable && p.wireless
                  ? "Not available over the USB cable yet"
                  : p.board && !board
                    ? "Opens once the keyboard has been read"
                    : undefined
              }
              onClick={() => setPage(p.id)}
            >
              <p.icon /> {p.label}
            </Button>
          ))}
        </nav>
        <div className="text-muted-foreground mt-auto px-5 py-4 text-[11px] leading-snug">
          Protocol from aula-f75-hub (MIT). Not affiliated with AULA.
        </div>
      </aside>

      <div className="flex min-w-0 flex-1 flex-col">
        <header className="flex items-center gap-3 border-b px-6 py-3">
          <h1 className="text-base font-medium">{PAGES.find((p) => p.id === shown)?.label}</h1>
          <div className="ml-auto flex items-center gap-2">
            {playing && (
              <div className="bg-primary/10 border-primary/30 flex items-center gap-1 rounded-full border py-0.5 pr-0.5 pl-2.5 text-xs">
                <span className="relative flex size-2">
                  <span className="bg-primary absolute inline-flex size-full animate-ping rounded-full opacity-60" />
                  <span className="bg-primary relative inline-flex size-2 rounded-full" />
                </span>
                <button
                  type="button"
                  className="hover:text-foreground text-muted-foreground px-1 font-medium"
                  onClick={() => setPage(PLAYING_PAGE[playing.kind])}
                  title="Lighting the keyboard now. Open its page"
                >
                  {playing.label}
                </button>
                <Button size="icon-xs" variant="ghost" className="rounded-full" onClick={stopPlaying} title="Stop: the keyboard goes back to its own effect">
                  <Square className="size-3 fill-current" />
                </Button>
              </div>
            )}
            <Select value={connection} onValueChange={(v) => chooseConnection(v as Connection)} disabled={busy || loading}>
              <SelectTrigger size="sm" title="Which connection to use">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {CONNECTIONS.map((c) => (
                  <SelectItem key={c.id} value={c.id}>
                    {c.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            {s75Mode ? (
              <Badge variant="secondary" className="gap-1.5" title="AULA S75 Pro">
                <span className="size-2 rounded-full bg-emerald-500" /> S75 Pro · {status?.s75}
              </Badge>
            ) : board ? (
              <Badge variant="secondary" className="gap-1.5">
                <span className="size-2 rounded-full bg-emerald-500" /> {board.mode}
              </Badge>
            ) : supported ? (
              <Badge variant="outline">{supported.mode} found</Badge>
            ) : (
              <Badge variant="outline" className="text-muted-foreground">
                Not connected
              </Badge>
            )}
            {updating && (
              <span className="text-muted-foreground flex items-center gap-1.5 text-xs" title="Over Bluetooth only what this page needs is read; the rest shows what was last read">
                <Loader2 className="size-3.5 animate-spin" />
                {progress ? `Reading ${progress.what}…` : "Updating…"}
              </span>
            )}
            {board?.battery != null && <Battery pct={board.battery} />}
            <Button size="sm" variant="ghost" disabled={busy || loading} onClick={reload} title="Read everything from the keyboard again">
              <RefreshCw className={cn(loading && "animate-spin")} /> Refresh
            </Button>
          </div>
        </header>

        <Dialog open={welcome} onOpenChange={(o) => !o && closeWelcome()}>
          <DialogContent className="sm:max-w-lg">
            <DialogHeader>
              <DialogTitle className="flex items-center gap-3">
                <img src="/logo.svg" alt="" className="size-9" /> Welcome to Lumen75
              </DialogTitle>
              <DialogDescription>An unofficial app for your AULA F75. Three things before you start:</DialogDescription>
            </DialogHeader>
            <ol className="grid gap-4 text-sm">
              <li className="grid gap-2">
                <span className="font-medium">1. How is the keyboard connected?</span>
                <Select value={connection} onValueChange={(v) => chooseConnection(v as Connection)}>
                  <SelectTrigger className="w-56">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {CONNECTIONS.map((c) => (
                      <SelectItem key={c.id} value={c.id}>
                        {c.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
                <span className="text-muted-foreground text-xs">
                  Auto finds it on the cable, the 2.4G dongle or Bluetooth (pair “AULA-F75 5.0” in Windows settings first).
                </span>
              </li>
              <li className="grid gap-1">
                <span className="font-medium">2. Keep the official AULA app closed</span>
                {status?.vendor_app_running ? (
                  <span className="text-destructive">
                    It's running now. Quit it, also from the tray: both apps talk to the keyboard over the same channel.
                  </span>
                ) : (
                  <span className="text-muted-foreground">Both apps talk to the keyboard over the same channel, so use one at a time.</span>
                )}
              </li>
              <li className="grid gap-1">
                <span className="font-medium">3. Make a backup first</span>
                <span className="text-muted-foreground">
                  Device → Backup saves all the keyboard's settings to a folder, so you can always go back.
                </span>
              </li>
            </ol>
            <DialogFooter>
              <Button
                variant="outline"
                disabled={!board}
                title={board ? undefined : "Once the keyboard has been read"}
                onClick={() => {
                  setPage("device");
                  closeWelcome();
                }}
              >
                Back up now
              </Button>
              <Button onClick={closeWelcome}>Get started</Button>
            </DialogFooter>
          </DialogContent>
        </Dialog>

        <main className="flex-1 overflow-y-auto p-6">
          <div className="mx-auto grid max-w-5xl gap-4">
            {status?.vendor_app_running && (
              <Alert variant="destructive">
                <TriangleAlert />
                <AlertTitle>The official AULA app is running</AlertTitle>
                <AlertDescription>It uses the same channel. Quit it (also from the tray) before changing settings here.</AlertDescription>
              </Alert>
            )}

            {waitsForBoard && (
              <Card>
                <CardHeader>
                  <CardTitle>{loading ? "Reading the keyboard…" : "Can't reach the keyboard"}</CardTitle>
                  <CardDescription>
                    {loading
                      ? "This takes a few seconds over 2.4G or the cable, and longer over Bluetooth. Animations, Audio, Games and Discord work meanwhile."
                      : error ??
                        "Plug in the dongle and switch the keyboard to 2.4G, or to Bluetooth once it is paired in Windows settings. Then press a key to wake it. Or plug in the cable with the switch on wired."}
                  </CardDescription>
                </CardHeader>
                <CardContent>
                  {loading ? (
                    <div className="grid gap-2">
                      <div className="text-muted-foreground flex items-center gap-2 text-sm">
                        <Loader2 className="size-4 animate-spin" />
                        {progress ? `Reading ${progress.what} (${progress.step} of ${progress.total})` : "Connecting…"}
                      </div>
                      <div className="bg-muted h-1.5 overflow-hidden rounded-full">
                        <div
                          className="bg-primary h-full rounded-full transition-[width] duration-500"
                          style={{ width: `${progress ? ((progress.step - 0.5) / progress.total) * 100 : 3}%` }}
                        />
                      </div>
                    </div>
                  ) : (
                    <Button onClick={reload}>
                      <RefreshCw /> Try again
                    </Button>
                  )}
                </CardContent>
              </Card>
            )}

            {s75Mode && (
              <>
                {shown === "lighting" && <S75LightingPage />}
                {shown === "perkey" && <S75PerKeyPage />}
                {shown === "keys" && <S75KeysPage />}
                {shown === "macros" && <S75MacrosPage />}
                {shown === "screen" && <S75ScreenPage />}
                {shown === "device" && <S75DevicePage />}
              </>
            )}
            {!s75Mode && catalog && (
              <>
                {shown === "animations" && <AnimationsPage catalog={catalog} />}
                {shown === "audio" && <AudioPage catalog={catalog} />}
                {shown === "games" && <GamesPage catalog={catalog} />}
                {shown === "discord" && <DiscordPage catalog={catalog} />}
              </>
            )}
            {!s75Mode && board && catalog && (
              <>
                {shown === "lighting" && <LightingPage board={board} catalog={catalog} busy={busy} run={run} />}
                {shown === "perkey" && <PerKeyPage board={board} catalog={catalog} busy={busy} run={run} />}
                {shown === "keys" && <KeysPage board={board} catalog={catalog} busy={busy} run={run} />}
                {shown === "macros" && <MacrosPage board={board} catalog={catalog} busy={busy} run={run} />}
                {shown === "device" && (
                  <DevicePage board={board} catalog={catalog} busy={busy} run={run} setBusy={setBusyBoth} reload={reload} />
                )}
              </>
            )}
          </div>
        </main>
      </div>
    </div>
  );
}

function Battery({ pct }: { pct: number }) {
  const Icon = pct > 60 ? BatteryFull : pct > 25 ? BatteryMedium : BatteryLow;
  return (
    <Badge variant="outline" className="gap-1" title="Battery (experimental reading)">
      <Icon className="size-3.5" /> {pct}%
    </Badge>
  );
}
