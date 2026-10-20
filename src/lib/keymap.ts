import type { Catalog, Entry, MacroView } from "./api";

/** Consumer-control usages the factory Fn layer uses (type 0x02 entries). */
const CONSUMER: Record<number, string> = {
  0xb5: "Next track",
  0xb6: "Previous track",
  0xb7: "Stop",
  0xcd: "Play / pause",
  0xe2: "Mute",
  0xe9: "Volume up",
  0xea: "Volume down",
  0x223: "Browser home",
  0x18a: "Mail",
  0x192: "Calculator",
};

/** Fn-layer special functions (type 0x07), from aula-f75-hub's protocol notes. */
const SPECIAL: Record<number, string> = {
  0x01: "Windows key lock",
  0x04: "Factory reset (hold)",
  0x05: "Bluetooth device 1",
  0x06: "Bluetooth device 2",
  0x07: "Bluetooth device 3",
  0x08: "2.4G",
  0x0a: "(unknown)",
  0x11: "Battery gauge",
  0x18: "Windows mode",
  0x19: "Android mode",
  0x1a: "Mac mode",
};

/** Lighting functions (type 0x08). */
function lightingFn(e: Entry): string {
  const [, a, b] = e;
  if (a === 0 && b === 0 && e[3] === 0) return "Next lighting effect";
  if (a === 0 && e[3] === 1) return "Next side light mode";
  if (a === 2) return "Next effect colour";
  if (a === 3) return b === 1 ? "Brightness up" : "Brightness down";
  if (a === 4) return b === 1 ? "Speed up" : "Speed down";
  return "Lighting function";
}

export interface Describe {
  text: string;
  kind: "key" | "modifier" | "combo" | "none" | "media" | "macro" | "special" | "lighting" | "fn" | "raw";
}

export function describeEntry(e: Entry, cat: Catalog, macros: MacroView[]): Describe {
  const [type, mods, hi, lo] = e;
  const modNames = cat.modifiers.filter(([, , bit]) => mods & bit).map(([, label]) => label.replace("Left ", "").replace("Right ", "R"));
  switch (type) {
    case 0x00: {
      const key = cat.hid.find(([, , u]) => u === lo)?.[1];
      if (!lo && !mods) return { text: "Disabled", kind: "none" };
      if (!lo) return { text: modNames.join(" + "), kind: "modifier" };
      if (!mods) return { text: key ?? `Usage 0x${lo.toString(16)}`, kind: "key" };
      return { text: [...modNames, key ?? `0x${lo.toString(16)}`].join(" + "), kind: "combo" };
    }
    case 0x02: {
      const usage = (hi << 8) | lo;
      return { text: CONSUMER[usage] ?? `Media 0x${usage.toString(16)}`, kind: "media" };
    }
    case 0x03: {
      const name = macros[lo]?.name ?? `#${lo}`;
      const how = mods === 1 ? (hi > 1 ? ` ×${hi}` : "") : mods === 2 ? " (while held)" : mods === 4 ? " (until a key)" : "";
      return { text: `Macro “${name}”${how}`, kind: "macro" };
    }
    case 0x07:
      return { text: SPECIAL[lo] ?? `Special 0x${lo.toString(16)}`, kind: "special" };
    case 0x08:
      return { text: lightingFn(e), kind: "lighting" };
    case 0x0d:
      return { text: "Fn", kind: "fn" };
    default:
      return { text: e.map((b) => b.toString(16).padStart(2, "0")).join(" "), kind: "raw" };
  }
}

/** Short text for a keycap (second line). */
export function shortEntry(e: Entry, cat: Catalog, macros: MacroView[]): string {
  const d = describeEntry(e, cat, macros);
  if (d.kind === "macro") return "M:" + (macros[e[3]]?.name ?? e[3]);
  if (d.kind === "none") return "off";
  return d.text.length > 9 ? d.text.slice(0, 8) + "…" : d.text;
}
