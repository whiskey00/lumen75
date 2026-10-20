import { open } from "@tauri-apps/plugin-dialog";
import { toast } from "sonner";
import { Download, Upload } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { Separator } from "@/components/ui/separator";
import { useEffect, useState } from "react";
import { api, AppSettings, Board, Catalog } from "@/lib/api";
import type { Run } from "@/lib/run";

const cap = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);
const SIDE_HELP = ["Off", "Fills top to bottom", "Breathing", "Always on"];
const KNOB_HELP = ["Turn = volume, press = mute", "Turn = brightness, press = next effect"];

function sleepText(s: number) {
  return s === 0 ? "Never" : `${s / 60} min`;
}

export function DevicePage(props: {
  board: Board;
  catalog: Catalog;
  busy: boolean;
  run: Run;
  setBusy: (b: boolean) => void;
  reload: () => Promise<void>;
}) {
  const { board, catalog, busy, run, setBusy, reload } = props;
  const d = board.device;

  const pickFolder = async (title: string) => {
    const folder = await open({ directory: true, title });
    return typeof folder === "string" ? folder : null;
  };

  const backup = async () => {
    const folder = await pickFolder("Folder for the backup");
    if (!folder) return;
    setBusy(true);
    try {
      const saved = await api.backup(folder);
      toast.success(`Backup saved (${saved.length} tables)`, { description: folder });
    } catch (e) {
      toast.error("Backup failed", { description: String(e) });
    } finally {
      setBusy(false);
    }
  };

  const restore = async () => {
    const folder = await pickFolder("Backup folder to restore");
    if (!folder) return;
    setBusy(true);
    try {
      const done = await api.restore(folder);
      toast.success(`Restored ${done.length} tables`, { description: folder });
      await reload();
    } catch (e) {
      toast.error("Restore failed", { description: String(e) });
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="grid gap-4 lg:grid-cols-2">
      <Card>
        <CardHeader>
          <CardTitle>Settings</CardTitle>
          <CardDescription>Stored on the keyboard.</CardDescription>
        </CardHeader>
        <CardContent className="grid gap-5">
          <Setting label="Side light" help={SIDE_HELP[d.side_mode]}>
            <ToggleGroup
              type="single"
              variant="outline"
              value={String(d.side_mode)}
              disabled={busy}
              onValueChange={(v) => v && run(`Side light: ${catalog.side_modes[+v]}`, () => api.setDevice({ side_mode: +v }))}
            >
              {catalog.side_modes.map((m, i) => (
                <ToggleGroupItem key={m} value={String(i)} size="sm">
                  {cap(m)}
                </ToggleGroupItem>
              ))}
            </ToggleGroup>
          </Setting>
          <Separator />
          <Setting label="Knob" help={KNOB_HELP[d.knob_mode]}>
            <ToggleGroup
              type="single"
              variant="outline"
              value={String(d.knob_mode)}
              disabled={busy}
              onValueChange={(v) => v && run(`Knob: ${catalog.knob_modes[+v]}`, () => api.setDevice({ knob_mode: +v }))}
            >
              {catalog.knob_modes.map((m, i) => (
                <ToggleGroupItem key={m} value={String(i)} size="sm">
                  {cap(m)}
                </ToggleGroupItem>
              ))}
            </ToggleGroup>
          </Setting>
          <Separator />
          <Setting label="OS mode" help="Same as Fn+W / Fn+Q / Fn+E">
            <ToggleGroup
              type="single"
              variant="outline"
              value={String(d.os_mode)}
              disabled={busy}
              onValueChange={(v) => v && run(`OS mode: ${catalog.os_modes[+v]}`, () => api.setDevice({ os_mode: +v }))}
            >
              {catalog.os_modes.map((m, i) => (
                <ToggleGroupItem key={m} value={String(i)} size="sm">
                  {cap(m)}
                </ToggleGroupItem>
              ))}
            </ToggleGroup>
          </Setting>
          <Separator />
          <Setting label="Lock Windows key" help="Same as Fn+Win">
            <Switch
              checked={d.win_lock}
              disabled={busy}
              onCheckedChange={(v) => run(`Windows key ${v ? "locked" : "unlocked"}`, () => api.setDevice({ win_lock: v }))}
            />
          </Setting>
          <Separator />
          <Setting label="Sleep after" help="2.4G and Bluetooth only">
            <Select
              value={String(d.sleep)}
              disabled={busy}
              onValueChange={(v) => run(`Sleep: ${sleepText(+v)}`, () => api.setDevice({ sleep: +v }))}
            >
              <SelectTrigger className="w-32">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {[...new Set([...catalog.sleep_choices, d.sleep])]
                  .sort((a, b) => a - b)
                  .map((s) => (
                    <SelectItem key={s} value={String(s)}>
                      {sleepText(s)}
                    </SelectItem>
                  ))}
              </SelectContent>
            </Select>
          </Setting>
        </CardContent>
      </Card>

      <div className="grid content-start gap-4">
      <AppCard />
      <Card className="self-start">
        <CardHeader>
          <CardTitle>Backup</CardTitle>
          <CardDescription>
            Keymaps (all four layers), lighting, colour palettes and per-key colours, as hex files. Same format as{" "}
            <code>aula backup</code>, so backups work with both tools.
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-wrap gap-2">
          <Button variant="secondary" disabled={busy} onClick={backup}>
            <Download /> Back up…
          </Button>
          <Button variant="outline" disabled={busy} onClick={restore}>
            <Upload /> Restore…
          </Button>
        </CardContent>
      </Card>
      </div>
    </div>
  );
}

const IDLE_CHOICES = [0, 1, 2, 5, 10, 15, 30, 60];

/** App behaviour: start with Windows, keep running in the tray, pause animations when idle. */
function AppCard() {
  const [s, setS] = useState<AppSettings | null>(null);
  useEffect(() => {
    api.appSettingsGet().then(setS).catch(() => {});
  }, []);
  if (!s) return null;
  const change = async (c: Partial<AppSettings>) => {
    try {
      setS(await api.appSettingsSet(c));
    } catch (e) {
      toast.error("App setting", { description: String(e) });
    }
  };
  return (
    <Card>
      <CardHeader>
        <CardTitle>App</CardTitle>
        <CardDescription>Animations and audio lighting run while AULA Studio does.</CardDescription>
      </CardHeader>
      <CardContent className="grid gap-5">
        <Setting label="Start with Windows" help="Starts hidden in the tray and resumes your animation">
          <Switch checked={s.start_with_windows} onCheckedChange={(v) => change({ start_with_windows: v })} />
        </Setting>
        <Separator />
        <Setting label="Keep running in the tray" help="Closing the window doesn't quit">
          <Switch checked={s.close_to_tray} onCheckedChange={(v) => change({ close_to_tray: v })} />
        </Setting>
        <Separator />
        <Setting label="Pause animations when idle" help="Lets the keyboard sleep; a key press resumes">
          <Select value={String(s.idle_pause_minutes)} onValueChange={(v) => change({ idle_pause_minutes: +v })}>
            <SelectTrigger className="w-32">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {[...new Set([...IDLE_CHOICES, s.idle_pause_minutes])]
                .sort((a, b) => a - b)
                .map((m) => (
                  <SelectItem key={m} value={String(m)}>
                    {m === 0 ? "Never" : `After ${m} min`}
                  </SelectItem>
                ))}
            </SelectContent>
          </Select>
        </Setting>
      </CardContent>
    </Card>
  );
}

function Setting({ label, help, children }: { label: string; help?: string; children: React.ReactNode }) {
  return (
    <div className="flex flex-wrap items-center justify-between gap-3">
      <div className="grid gap-0.5">
        <Label>{label}</Label>
        {help && <span className="text-muted-foreground text-xs">{help}</span>}
      </div>
      {children}
    </div>
  );
}
