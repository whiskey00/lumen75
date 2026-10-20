import type { Snapshot } from "./api";

/** Runs one keyboard command, shows its outcome and merges what it returns. */
export type Run = (label: string, op: () => Promise<Snapshot>) => Promise<boolean>;
