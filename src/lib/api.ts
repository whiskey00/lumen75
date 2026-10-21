import { invoke } from "@tauri-apps/api/core";

export type Rgb = [number, number, number];
export type Entry = [number, number, number, number];

export interface Key {
  index: number;
  name: string;
  label: string;
  /** x1, y1, x2, y2 in the official app's picture; all zero = not drawn */
  rect: [number, number, number, number];
}

export interface EffectInfo {
  hw: number;
  name: string;
  label: string;
  speed: boolean;
  color: boolean;
}

export interface EffectSlot {
  /** 0-9 */
  brightness: number;
  /** 0-4 */
  speed: number;
  /** 0-6 palette colour, 7 = multicolour */
  color: number;
}

export interface EffectView extends EffectInfo {
  slot: EffectSlot | null;
  palette: Rgb[] | null;
}

export interface DeviceSettings {
  side_mode: number;
  knob_mode: number;
  os_mode: number;
  win_lock: boolean;
  sleep: number;
}

export interface MacroView {
  name: string;
  events: string[];
  event_count: number;
}

/** What a command read or changed; absent fields are unchanged. */
export interface Snapshot {
  mode?: string | null;
  battery?: number | null;
  active_effect?: number | null;
  effects?: EffectView[] | null;
  device?: DeviceSettings | null;
  custom?: Rgb[] | null;
  keymap?: Entry[][] | null;
  factory_keymap?: Entry[][] | null;
  macros?: MacroView[] | null;
}

/** Everything the UI shows, once read from the keyboard. */
export interface Board {
  mode: string;
  battery: number | null;
  active_effect: number;
  effects: EffectView[];
  device: DeviceSettings;
  custom: Rgb[];
  keymap: Entry[][];
  factory_keymap: Entry[][];
  macros: MacroView[];
}

export function merge(board: Board | null, s: Snapshot): Board | null {
  const defined = Object.fromEntries(Object.entries(s).filter(([, v]) => v !== null && v !== undefined));
  if (!board) {
    // over the USB cable the per-key colours and macros aren't read (yet)
    return "effects" in defined && "keymap" in defined ? ({ battery: null, custom: [] as Rgb[], macros: [] as MacroView[], ...defined } as Board) : null;
  }
  return { ...board, ...defined };
}

export interface Catalog {
  keys: Key[];
  effects: EffectInfo[];
  colors: string[];
  side_modes: string[];
  knob_modes: string[];
  os_modes: string[];
  sleep_choices: number[];
  /** name, label, HID usage */
  hid: [string, string, number][];
  /** name, label, modifier bit */
  modifiers: [string, string, number][];
  layout: string;
}

export interface Status {
  devices: { mode: string; supported: boolean; name: string }[];
  vendor_app_running: boolean;
  connected: boolean;
}

export interface EffectChange {
  hw: number;
  brightness?: number;
  speed?: number;
  color?: number;
  rgb?: Rgb;
}

export interface DeviceChange {
  side_mode?: number;
  knob_mode?: number;
  os_mode?: number;
  win_lock?: boolean;
  sleep?: number;
}

export interface MacroSave {
  name: string;
  steps: string[];
  delay: number;
  key: [number, number] | null;
  mode: number;
  count: number;
}

export type AudioMode = "spectrum" | "mirror" | "ripple" | "pulse";

export interface AudioSettings {
  mode: AudioMode;
  /** 0.2 - 4.0 */
  gain: number;
  /** 0 - 9 */
  smoothness: number;
  /** Rainbow colours; off = your `colors`. */
  colorful: boolean;
  /** Kept for older saved settings: the first of `colors`. */
  color: Rgb;
  /** 1-7 colours, spread over the bands (used when not colourful). */
  colors: Rgb[];
  /** Output device to listen to; null = the Windows default. */
  device: string | null;
}

/** One streamed frame: band levels (0..1) and the key colours shown. */
export interface AudioFrame {
  levels: number[];
  keys: [number, Rgb][];
  /** No sound arrived from the device for a second. */
  silent: boolean;
  device: string;
}

export type AnimKind = "stream" | "breathing" | "wave" | "sparkle" | "gradient";
export type AnimDirection = "right" | "left" | "up" | "down";

export interface AnimSettings {
  kind: AnimKind;
  /** 1-7 colours */
  colors: Rgb[];
  /** 0.1 - 3 */
  speed: number;
  /** 0.05 - 1 */
  brightness: number;
  direction: AnimDirection;
}

export interface AnimState {
  running: boolean;
  settings: AnimSettings;
}

export interface AnimFrame {
  keys: [number, Rgb][];
  /** Paused because the computer is idle. */
  paused: boolean;
}

/** Which connection the app looks for. */
export type Connection = "auto" | "cable" | "dongle" | "bluetooth";

export interface AppSettings {
  start_with_windows: boolean;
  close_to_tray: boolean;
  idle_pause_minutes: number;
  connection: Connection;
}

export interface Presets {
  animations: { name: string; settings: AnimSettings }[];
  perkey: { name: string; colors: Rgb[] }[];
  audio: { name: string; settings: AudioSettings }[];
}

export const api = {
  presetsGet: () => invoke<Presets>("presets_get"),
  animPresetSave: (name: string, settings: AnimSettings) => invoke<Presets>("anim_preset_save", { name, settings }),
  perkeyPresetSave: (name: string, colors: Rgb[]) => invoke<Presets>("perkey_preset_save", { name, colors }),
  audioPresetSave: (name: string, settings: AudioSettings) => invoke<Presets>("audio_preset_save", { name, settings }),
  presetDelete: (kind: "animation" | "perkey" | "audio", name: string) => invoke<Presets>("preset_delete", { kind, name }),
  animGet: () => invoke<AnimState>("anim_get"),
  animStart: (settings: AnimSettings) => invoke<AnimState>("anim_start", { settings }),
  animUpdate: (settings: AnimSettings) => invoke<void>("anim_update", { settings }),
  animStop: () => invoke<AnimState>("anim_stop"),
  appSettingsGet: () => invoke<AppSettings>("app_settings_get"),
  appSettingsSet: (change: Partial<AppSettings>) => invoke<AppSettings>("app_settings_set", { change }),
  audioStart: (settings: AudioSettings) => invoke<void>("audio_start", { settings }),
  audioUpdate: (settings: AudioSettings) => invoke<void>("audio_update", { settings }),
  audioStop: () => invoke<void>("audio_stop"),
  audioRunning: () => invoke<boolean>("audio_running"),
  audioDevices: () => invoke<{ devices: string[]; default: string | null }>("audio_devices"),
  catalog: () => invoke<Catalog>("catalog"),
  status: () => invoke<Status>("status"),
  readAll: () => invoke<Snapshot>("read_all"),
  readBattery: () => invoke<number | null>("read_battery"),
  setEffect: (change: EffectChange) => invoke<Snapshot>("set_effect", { change }),
  resetPalette: (hw: number) => invoke<Snapshot>("reset_palette", { hw }),
  setCustom: (colors: Rgb[]) => invoke<Snapshot>("set_custom", { colors }),
  setDevice: (change: DeviceChange) => invoke<Snapshot>("set_device", { change }),
  setKeys: (layer: number, changes: [number, Entry][]) => invoke<Snapshot>("set_keys", { layer, changes }),
  resetKeys: (layer: number, indices: number[]) => invoke<Snapshot>("reset_keys", { layer, indices }),
  saveMacro: (m: MacroSave) => invoke<Snapshot>("save_macro", { m }),
  deleteMacro: (index: number) => invoke<Snapshot>("delete_macro", { index }),
  backup: (folder: string) => invoke<string[]>("backup", { folder }),
  restore: (folder: string) => invoke<string[]>("restore", { folder }),
};

export const toHex = ([r, g, b]: Rgb) => "#" + [r, g, b].map((v) => v.toString(16).padStart(2, "0")).join("");

export const fromHex = (hex: string): Rgb => {
  const h = hex.replace("#", "");
  return [0, 2, 4].map((i) => parseInt(h.slice(i, i + 2), 16)) as Rgb;
};

/** A typed hex code ("#f80", "ff8800", any case) as "#rrggbb", or null if it isn't one. */
export const parseHex = (text: string): string | null => {
  const h = text.trim().replace(/^#/, "").toLowerCase();
  if (/^[0-9a-f]{3}$/.test(h)) return "#" + [...h].map((c) => c + c).join("");
  return /^[0-9a-f]{6}$/.test(h) ? "#" + h : null;
};

export const sameEntry = (a: Entry, b: Entry) => a.every((v, i) => v === b[i]);
