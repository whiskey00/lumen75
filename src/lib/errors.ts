/**
 * An error from the app's backend, said plainly with what to do next. Messages that are
 * already plain pass through; low-level ones (packet numbers, Windows error codes) get a
 * sentence you can act on.
 */
export function friendly(e: unknown): string {
  const raw = String(e).replace(/^Error:\s*/, "").trim();
  const has = (...words: string[]) => words.some((w) => raw.toLowerCase().includes(w.toLowerCase()));

  if (has("incomplete (packets", "returned", "got no matching reply"))
    return "Some of the keyboard's reply got lost on the way (this happens most over Bluetooth). Try again.";
  if (has("not acknowledged"))
    return "The keyboard didn't confirm the change. It may be asleep: press a key to wake it, then try again.";
  if (has("timed out"))
    return "The keyboard didn't respond in time. Press a key to wake it, then try again.";
  if (has("rejected"))
    return "The keyboard refused that change. Read it again (Refresh) and try once more.";
  if (has("didn't keep the change"))
    return "The keyboard didn't keep the change. Try again.";
  if (has("access denied", "Windows error 5)"))
    return "Windows won't let Lumen75 open the keyboard. Close the official AULA app (also from the tray) and other keyboard tools, then try again.";
  if (has("Windows error 1167", "Windows error 31)", "Windows error 22)", "Windows error 2)"))
    return "The keyboard was disconnected. Check the dongle or cable, switch it back on, then try again.";
  if (has("Windows error"))
    return `Windows couldn't talk to the keyboard (${raw.match(/Windows error \d+/)?.[0]}). Unplug the dongle or cable, plug it back in, then try again.`;
  if (has("lock poisoned"))
    return "Something went wrong inside Lumen75. Quit it from the tray and start it again.";
  if (has("frame too long", "must be"))
    return `Lumen75 made a frame the keyboard can't take. Please report this: ${raw}`;
  return raw;
}
