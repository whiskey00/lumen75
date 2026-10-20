import { useState } from "react";
import { Bookmark, Save, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import {
  AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription, AlertDialogFooter,
  AlertDialogHeader, AlertDialogTitle,
} from "@/components/ui/alert-dialog";

export interface PresetItem {
  name: string;
  /** A few colours to show on the chip. */
  swatch: string[];
}

interface Props {
  title: string;
  items: PresetItem[];
  /** What "save" stores, for the dialog's description. */
  saves: string;
  onLoad: (name: string) => void;
  onSave: (name: string) => Promise<boolean>;
  onDelete: (name: string) => Promise<void>;
}

/** Your own saved presets: click to load, save the current one under a name, delete. */
export function PresetBar({ title, items, saves, onLoad, onSave, onDelete }: Props) {
  const [saving, setSaving] = useState(false);
  const [name, setName] = useState("");
  const [deleting, setDeleting] = useState<string | null>(null);
  const exists = items.some((p) => p.name.toLowerCase() === name.trim().toLowerCase());

  const save = async () => {
    if (await onSave(name)) {
      setSaving(false);
      setName("");
    }
  };

  return (
    <div className="flex flex-wrap items-center gap-2">
      <span className="text-muted-foreground flex items-center gap-1 text-sm">
        <Bookmark className="size-3.5" /> {title}:
      </span>
      {items.length === 0 && <span className="text-muted-foreground text-sm italic">none yet</span>}
      {items.map((p) => (
        <div key={p.name} className="group relative">
          <Button size="sm" variant="secondary" className="gap-2 pr-3" onClick={() => onLoad(p.name)}>
            <span className="flex">
              {p.swatch.slice(0, 5).map((c, i) => (
                <span key={i} className="-ml-1 size-3 rounded-full border border-black/40 first:ml-0" style={{ background: c }} />
              ))}
            </span>
            {p.name}
          </Button>
          <button
            aria-label={`Delete preset ${p.name}`}
            onClick={() => setDeleting(p.name)}
            className="bg-background absolute -top-2 -right-2 hidden rounded-full border p-0.5 group-hover:block"
          >
            <X className="size-3" />
          </button>
        </div>
      ))}
      <Button size="sm" variant="outline" onClick={() => setSaving(true)}>
        <Save /> Save as preset…
      </Button>

      <Dialog open={saving} onOpenChange={setSaving}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>Save as preset</DialogTitle>
            <DialogDescription>Saves {saves}. Saving under an existing name replaces that preset.</DialogDescription>
          </DialogHeader>
          <div className="grid gap-2">
            <Label htmlFor="preset-name">Name</Label>
            <Input
              id="preset-name"
              autoFocus
              maxLength={40}
              value={name}
              onChange={(e) => setName(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && name.trim() && save()}
              placeholder="e.g. Cherry blossom"
            />
          </div>
          <DialogFooter>
            <Button variant="ghost" onClick={() => setSaving(false)}>
              Cancel
            </Button>
            <Button disabled={!name.trim()} onClick={save}>
              {exists ? "Replace" : "Save"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <AlertDialog open={deleting !== null} onOpenChange={(o) => !o && setDeleting(null)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Delete “{deleting}”?</AlertDialogTitle>
            <AlertDialogDescription>The preset is removed. What's on the keyboard doesn't change.</AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction
              onClick={() => {
                const n = deleting!;
                setDeleting(null);
                onDelete(n);
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
