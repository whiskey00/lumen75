import { useEffect, useState } from "react";
import { toast } from "sonner";
import { BatteryMedium } from "lucide-react";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { api, S75Settings } from "@/lib/api";
import { friendly } from "@/lib/errors";
import { S75Untested, useS75 } from "@/pages/s75-lighting";

export function S75DevicePage({ children }: { children?: React.ReactNode }) {
  const [state, setState] = useS75();
  const [battery, setBattery] = useState<number | null>(null);

  useEffect(() => {
    if (state?.connection === "2.4G dongle") api.s75Battery().then(setBattery).catch(() => {});
  }, [state?.connection]);

  if (!state) return null;
  const s = state.saved.settings;

  const set = async (p: Partial<S75Settings>) => {
    const next = { ...s, ...p };
    setState({ ...state, saved: { ...state.saved, settings: next } });
    try {
      const saved = await api.s75SetSettings(next);
      setState((st) => (st ? { ...st, saved } : st));
    } catch (e) {
      toast.error("Can't change the setting", { description: friendly(e) });
    }
  };

  return (
    <div className="grid gap-4">
      <S75Untested />
      <Card>
        <CardHeader className="flex flex-row items-start justify-between gap-4">
          <div className="grid gap-1.5">
            <CardTitle>AULA S75 Pro</CardTitle>
            <CardDescription>{state.connection ? `Connected over the ${state.connection}.` : "Not connected."}</CardDescription>
          </div>
          {battery !== null && (
            <span className="text-muted-foreground flex items-center gap-1 text-sm">
              <BatteryMedium className="size-4" /> {battery}%
            </span>
          )}
        </CardHeader>
        <CardContent className="grid gap-5">
          <Toggle label="Lock the Windows key" checked={s.win_lock} onChange={(win_lock) => set({ win_lock })} />
          <Toggle label="Block Alt + F4" checked={s.alt_f4_lock} onChange={(alt_f4_lock) => set({ alt_f4_lock })} />
          <Toggle label="Block Alt + Tab" checked={s.alt_tab_lock} onChange={(alt_tab_lock) => set({ alt_tab_lock })} />
          <div className="grid gap-2">
            <Label>Fn layer</Label>
            <ToggleGroup type="single" variant="outline" value={String(s.fn_switch)} onValueChange={(v) => v && set({ fn_switch: Number(v) })} className="justify-start">
              <ToggleGroupItem value="0">While Fn is held</ToggleGroupItem>
              <ToggleGroupItem value="1">Fn toggles it</ToggleGroupItem>
            </ToggleGroup>
          </div>
          <div className="grid gap-2">
            <Label>Sleep after</Label>
            <Select value={String(s.sleep)} onValueChange={(v) => set({ sleep: Number(v) })}>
              <SelectTrigger className="w-44">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {state.catalog.sleep.map(([v, name]) => (
                  <SelectItem key={v} value={String(v)}>
                    {name}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <div className="grid gap-2">
            <Label>Key response (debounce)</Label>
            <ToggleGroup type="single" variant="outline" value={String(s.response)} onValueChange={(v) => v && set({ response: Number(v) })} className="justify-start">
              {[1, 2, 3, 4, 5].map((v) => (
                <ToggleGroupItem key={v} value={String(v)}>
                  {v}
                </ToggleGroupItem>
              ))}
            </ToggleGroup>
            <span className="text-muted-foreground text-xs">Lower reacts faster; raise it if keys double-type.</span>
          </div>
        </CardContent>
      </Card>
      {children}
    </div>
  );
}

function Toggle({ label, checked, onChange }: { label: string; checked: boolean; onChange: (v: boolean) => void }) {
  return (
    <div className="flex items-center justify-between gap-4">
      <Label>{label}</Label>
      <Switch checked={checked} onCheckedChange={onChange} aria-label={label} />
    </div>
  );
}
