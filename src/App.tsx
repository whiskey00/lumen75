import { useCallback, useEffect, useRef, useState } from "react";
import { toast } from "sonner";
import { AudioLines, BatteryFull, Sparkles, BatteryLow, BatteryMedium, Grid3x3, Keyboard as KeyboardIcon, Loader2, RefreshCw, Settings2, SquareFunction, Sun, TriangleAlert } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { cn } from "@/lib/utils";
import { api, Board, Catalog, Connection, merge, Snapshot, Status } from "@/lib/api";
import { LightingPage } from "@/pages/lighting";
import { PerKeyPage } from "@/pages/perkey";
import { KeysPage } from "@/pages/keys";
import { MacrosPage } from "@/pages/macros";
import { DevicePage } from "@/pages/device";
import { AudioPage } from "@/pages/audio";
import { AnimationsPage } from "@/pages/animations";

type Page = "lighting" | "perkey" | "animations" | "audio" | "keys" | "macros" | "device";

const PAGES: { id: Page; label: string; icon: typeof Sun; wireless?: boolean }[] = [
  { id: "lighting", label: "Lighting", icon: Sun },
  { id: "perkey", label: "Per-key", icon: Grid3x3, wireless: true },
  { id: "animations", label: "Animations", icon: Sparkles },
  { id: "audio", label: "Audio", icon: AudioLines },
  // over the cable the macros aren't read (the page needs them) and key writes don't work yet
  { id: "keys", label: "Keys", icon: KeyboardIcon, wireless: true },
  { id: "macros", label: "Macros", icon: SquareFunction, wireless: true },
  { id: "device", label: "Device", icon: Settings2 },
];

const CABLE = "USB cable";

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
  const busyRef = useRef(false);

  const refreshStatus = useCallback(() => api.status().then(setStatus).catch(() => {}), []);

  const reload = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const s = await api.readAll();
      setBoard(merge(null, s));
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
      refreshStatus();
    }
  }, [refreshStatus]);

  useEffect(() => {
    api.catalog().then(setCatalog);
    api.appSettingsGet().then((s) => setConnection(s.connection)).catch(() => {});
    reload();
  }, [reload]);

  const chooseConnection = async (c: Connection) => {
    setConnection(c);
    setBoard(null);
    try {
      await api.appSettingsSet({ connection: c });
    } catch (e) {
      setError(String(e));
      return;
    }
    reload();
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
      toast.error(label, { id, description: String(e), duration: 8000 });
      return false;
    } finally {
      setBusyBoth(false);
    }
  }, []);

  const wanted = CONNECTIONS.find((c) => c.id === connection)?.mode;
  const supported = status?.devices.find((d) => d.supported && (!wanted || d.mode === wanted));
  const onCable = board?.mode === CABLE;
  // a page the cable can't serve falls back to Lighting
  const shown: Page = onCable && PAGES.find((p) => p.id === page)?.wireless ? "lighting" : page;

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
          {PAGES.map((p) => (
            <Button
              key={p.id}
              variant={page === p.id ? "secondary" : "ghost"}
              className="justify-start"
              disabled={!board || (onCable && p.wireless)}
              title={onCable && p.wireless ? "Not available over the USB cable yet" : undefined}
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
            {board ? (
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
            {board?.battery != null && <Battery pct={board.battery} />}
            <Button size="sm" variant="ghost" disabled={busy || loading} onClick={reload} title="Read everything from the keyboard again">
              <RefreshCw className={cn(loading && "animate-spin")} /> Refresh
            </Button>
          </div>
        </header>

        <main className="flex-1 overflow-y-auto p-6">
          <div className="mx-auto grid max-w-5xl gap-4">
            {status?.vendor_app_running && (
              <Alert variant="destructive">
                <TriangleAlert />
                <AlertTitle>The official AULA app is running</AlertTitle>
                <AlertDescription>It uses the same channel. Quit it (also from the tray) before changing settings here.</AlertDescription>
              </Alert>
            )}

            {!board && (
              <Card>
                <CardHeader>
                  <CardTitle>{loading ? "Reading the keyboard…" : "Can't reach the keyboard"}</CardTitle>
                  <CardDescription>
                    {loading
                      ? "This takes a few seconds over 2.4G or the cable, and longer over Bluetooth."
                      : error ??
                        "Plug in the dongle and switch the keyboard to 2.4G, or to Bluetooth once it is paired in Windows settings. Then press a key to wake it. Or plug in the cable with the switch on wired."}
                  </CardDescription>
                </CardHeader>
                <CardContent>
                  {loading ? (
                    <Loader2 className="text-muted-foreground size-6 animate-spin" />
                  ) : (
                    <Button onClick={reload}>
                      <RefreshCw /> Try again
                    </Button>
                  )}
                </CardContent>
              </Card>
            )}

            {board && catalog && (
              <>
                {shown === "lighting" && <LightingPage board={board} catalog={catalog} busy={busy} run={run} />}
                {shown === "perkey" && <PerKeyPage board={board} catalog={catalog} busy={busy} run={run} />}
                {shown === "animations" && <AnimationsPage catalog={catalog} />}
                {shown === "audio" && <AudioPage catalog={catalog} />}
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
