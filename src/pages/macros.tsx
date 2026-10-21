import { useState } from "react";
import { Pencil, Plus, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import {
  AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription, AlertDialogFooter,
  AlertDialogHeader, AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { api, Board, Catalog } from "@/lib/api";
import type { Run } from "@/lib/run";

const NONE = "none";

export function MacrosPage({ board, catalog, busy, run }: { board: Board; catalog: Catalog; busy: boolean; run: Run }) {
  const [editing, setEditing] = useState<{ name: string } | null>(null);
  const [deleting, setDeleting] = useState<number | null>(null);

  /** Keys (on both layers) that run each macro. */
  const users = (i: number) =>
    [0, 1].flatMap((layer) =>
      catalog.keys
        .filter((k) => board.keymap[layer][k.index]?.[0] === 3 && board.keymap[layer][k.index][3] === i)
        .map((k) => (layer ? "Fn+" : "") + k.label),
    );

  return (
    <div className="grid gap-4">
      <Card>
        <CardHeader className="flex flex-row items-start justify-between gap-4">
          <div className="grid gap-1.5">
            <CardTitle>Macros</CardTitle>
            <CardDescription>
              Stored on the keyboard: they run even without this app. Text is typed for the {catalog.layout} layout.
            </CardDescription>
          </div>
          <Button disabled={busy} onClick={() => setEditing({ name: "" })}>
            <Plus /> New macro
          </Button>
        </CardHeader>
        <CardContent className="grid gap-3">
          {board.macros.length === 0 && <p className="text-muted-foreground text-sm">No macros stored.</p>}
          {board.macros.map((m, i) => {
            const keys = users(i);
            return (
              <div key={i} className="grid gap-2 rounded-lg border p-3">
                <div className="flex items-center gap-2">
                  <span className="font-medium">{m.name}</span>
                  {keys.length ? keys.map((k) => <Badge key={k}>{k}</Badge>) : <Badge variant="outline">not assigned</Badge>}
                  <span className="text-muted-foreground ml-auto text-xs">{m.event_count} events</span>
                  <Button size="icon-sm" variant="ghost" aria-label="Edit" disabled={busy} onClick={() => setEditing({ name: m.name })}>
                    <Pencil />
                  </Button>
                  <Button size="icon-sm" variant="ghost" aria-label="Delete" disabled={busy} onClick={() => setDeleting(i)}>
                    <Trash2 />
                  </Button>
                </div>
                <p className="text-muted-foreground font-mono text-xs leading-relaxed break-words">{m.events.join("  ")}</p>
              </div>
            );
          })}
        </CardContent>
      </Card>

      {editing && <MacroEditor initialName={editing.name} board={board} catalog={catalog} busy={busy} run={run} onClose={() => setEditing(null)} />}

      <AlertDialog open={deleting !== null} onOpenChange={(o) => !o && setDeleting(null)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Delete “{deleting !== null ? board.macros[deleting]?.name : ""}”?</AlertDialogTitle>
            <AlertDialogDescription>Keys that run it get their factory function back.</AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction
              onClick={() => {
                const i = deleting!;
                setDeleting(null);
                run(`Delete macro ${board.macros[i]?.name}`, () => api.deleteMacro(i));
              }}
            >
              Delete
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}

function MacroEditor(props: { initialName: string; board: Board; catalog: Catalog; busy: boolean; run: Run; onClose: () => void }) {
  const { initialName, catalog, busy, run, onClose } = props;
  const [name, setName] = useState(initialName);
  const [steps, setSteps] = useState(initialName ? "" : "text:Hello!");
  const [delay, setDelay] = useState(20);
  const [key, setKey] = useState(NONE);
  const [mode, setMode] = useState(1);
  const [count, setCount] = useState(1);

  const assignable = catalog.keys.filter((k) => k.name !== "fn");
  const save = async () => {
    const keyPair = key === NONE ? null : (key.split(":").map(Number) as [number, number]);
    const ok = await run(`Macro “${name.trim()}”`, () =>
      api.saveMacro({ name, steps: steps.split("\n"), delay, key: keyPair, mode, count }),
    );
    if (ok) onClose();
  };

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="sm:max-w-xl">
        <DialogHeader>
          <DialogTitle>{initialName ? `Replace “${initialName}”` : "New macro"}</DialogTitle>
          <DialogDescription>One step per line. Saving under an existing name replaces that macro.</DialogDescription>
        </DialogHeader>
        <div className="grid gap-4">
          <div className="grid gap-2">
            <Label htmlFor="mname">Name</Label>
            <Input id="mname" value={name} maxLength={30} onChange={(e) => setName(e.target.value)} placeholder="e.g. paste twice" />
          </div>
          <div className="grid gap-2">
            <Label htmlFor="msteps">Steps</Label>
            <Textarea id="msteps" rows={6} className="font-mono text-sm" value={steps} onChange={(e) => setSteps(e.target.value)} />
            <p className="text-muted-foreground text-xs leading-relaxed">
              <code>text:Hello world</code> types text · <code>ctrl+c</code> presses a combo · <code>wait:100</code> waits (ms) ·{" "}
              <code>down:shift</code> / <code>up:shift</code> hold and release · keys: <code>enter</code>, <code>tab</code>,{" "}
              <code>f5</code>, <code>left</code>…
            </p>
          </div>
          <div className="flex flex-wrap items-end gap-4">
            <div className="grid gap-2">
              <Label htmlFor="mdelay">Delay between events (ms)</Label>
              <Input id="mdelay" type="number" min={1} max={1000} value={delay} onChange={(e) => setDelay(Number(e.target.value))} className="w-28" />
            </div>
            <div className="grid gap-2">
              <Label>Assign to</Label>
              <Select value={key} onValueChange={setKey}>
                <SelectTrigger className="w-44">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value={NONE}>No key</SelectItem>
                  {assignable.map((k) => (
                    <SelectItem key={`1:${k.index}`} value={`1:${k.index}`}>
                      Fn + {k.label}
                    </SelectItem>
                  ))}
                  {assignable.map((k) => (
                    <SelectItem key={`0:${k.index}`} value={`0:${k.index}`}>
                      {k.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            {key !== NONE && (
              <div className="grid gap-2">
                <Label>Runs</Label>
                <Select value={String(mode)} onValueChange={(v) => setMode(Number(v))}>
                  <SelectTrigger className="w-44">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="1">a number of times</SelectItem>
                    <SelectItem value="2">while held</SelectItem>
                    <SelectItem value="4">until a key is pressed</SelectItem>
                  </SelectContent>
                </Select>
              </div>
            )}
            {key !== NONE && mode === 1 && (
              <div className="grid gap-2">
                <Label htmlFor="mcount">Times</Label>
                <Input id="mcount" type="number" min={1} max={255} value={count} onChange={(e) => setCount(Number(e.target.value))} className="w-20" />
              </div>
            )}
          </div>
          {key.startsWith("0:") && (
            <p className="text-sm text-amber-400">This replaces what the key normally types. Fn + a key is usually the safer choice.</p>
          )}
        </div>
        <DialogFooter>
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button disabled={busy || !name.trim() || !steps.trim()} onClick={save}>
            Save to keyboard
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
