import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import { toast } from "sonner";
import { Clock, ImageIcon, Loader2, Upload } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { api, ImageFit } from "@/lib/api";
import { friendly } from "@/lib/errors";
import { useS75 } from "@/pages/s75-lighting";

const FITS: { id: ImageFit; label: string }[] = [
  { id: "fill", label: "Fill" },
  { id: "fit", label: "Fit" },
  { id: "stretch", label: "Stretch" },
];

export function S75ScreenPage() {
  const [state] = useS75();
  const [path, setPath] = useState<string | null>(null);
  const [fit, setFit] = useState<ImageFit>("fill");
  const [preview, setPreview] = useState<string | null>(null);
  const [progress, setProgress] = useState<number | null>(null);
  const [clockBusy, setClockBusy] = useState(false);

  useEffect(() => {
    const off = listen<number>("s75-screen-progress", (e) => setProgress(e.payload));
    return () => void off.then((f) => f());
  }, []);

  useEffect(() => {
    if (!path) return;
    api
      .s75ScreenPreview(path, fit)
      .then(setPreview)
      .catch((e) => {
        setPreview(null);
        toast.error("Can't use that picture", { description: friendly(e) });
      });
  }, [path, fit]);

  if (!state) return null;
  const usb = state.connection === "USB cable";

  const choose = async () => {
    const p = await open({ title: "A GIF or picture for the screen", filters: [{ name: "Images", extensions: ["gif", "png", "jpg", "jpeg", "webp"] }] });
    if (typeof p === "string") setPath(p);
  };

  const upload = async () => {
    if (!path) return;
    setProgress(0);
    try {
      const frames = await api.s75UploadScreen(path, fit);
      toast.success("On the screen", { description: frames === 1 ? "A still picture." : `${frames} frames.` });
    } catch (e) {
      toast.error("Can't put it on the screen", { description: friendly(e) });
    } finally {
      setProgress(null);
    }
  };

  const syncClock = async () => {
    setClockBusy(true);
    try {
      await api.s75SyncClock();
      toast.success("The keyboard's clock now matches this PC");
    } catch (e) {
      toast.error("Can't set the clock", { description: friendly(e) });
    } finally {
      setClockBusy(false);
    }
  };

  return (
    <div className="grid gap-4">
      <Card>
        <CardHeader>
          <CardTitle>Screen</CardTitle>
          <CardDescription>
            A GIF or picture for the 135 × 240 screen (up to 141 frames). {usb ? "" : "Uploading needs the USB cable."}
          </CardDescription>
        </CardHeader>
        <CardContent className="grid gap-6 md:grid-cols-[auto_1fr]">
          <div className="bg-muted/40 flex h-60 w-[135px] items-center justify-center overflow-hidden rounded-md border">
            {preview ? <img src={preview} alt="How the screen will look" className="h-60 w-[135px]" /> : <ImageIcon className="text-muted-foreground size-6" />}
          </div>
          <div className="grid content-start gap-4">
            <Button variant="outline" className="justify-self-start" onClick={choose}>
              <ImageIcon /> {path ? "Choose another" : "Choose a GIF or picture"}
            </Button>
            {path && <span className="text-muted-foreground truncate text-sm">{path.split(/[\\/]/).pop()}</span>}
            <ToggleGroup type="single" variant="outline" value={fit} onValueChange={(v) => v && setFit(v as ImageFit)} className="justify-start">
              {FITS.map((f) => (
                <ToggleGroupItem key={f.id} value={f.id}>
                  {f.label}
                </ToggleGroupItem>
              ))}
            </ToggleGroup>
            <div className="flex items-center gap-3">
              <Button disabled={!path || !usb || progress !== null} onClick={upload}>
                {progress !== null ? <Loader2 className="animate-spin" /> : <Upload />}
                Put it on the screen
              </Button>
              {progress !== null && <span className="text-muted-foreground text-sm tabular-nums">{Math.round(progress * 100)}%</span>}
            </div>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">Clock</CardTitle>
          <CardDescription>The screen can show the time. Set it from this PC.</CardDescription>
        </CardHeader>
        <CardContent>
          <Button variant="outline" disabled={clockBusy} onClick={syncClock}>
            <Clock /> Set the clock now
          </Button>
        </CardContent>
      </Card>
    </div>
  );
}
