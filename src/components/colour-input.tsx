import { useEffect, useState } from "react";
import { Input } from "@/components/ui/input";
import { parseHex } from "@/lib/api";
import { cn } from "@/lib/utils";

/**
 * A colour swatch with its hex code, which can also be typed (#rgb or #rrggbb). The colour
 * changes as soon as the code is valid; leaving the field puts back the current colour.
 */
export function ColourInput({
  value,
  onChange,
  label,
  stacked = false,
}: {
  value: string;
  onChange: (hex: string) => void;
  label: string;
  /** Code under the swatch instead of beside it. */
  stacked?: boolean;
}) {
  const [text, setText] = useState(value);
  useEffect(() => setText(value), [value]);

  return (
    <div className={cn("flex gap-1.5", stacked ? "flex-col" : "items-center")}>
      <input
        type="color"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className={cn("cursor-pointer rounded-md border bg-transparent", stacked ? "h-11 w-20" : "h-9 w-14")}
        aria-label={label}
      />
      <Input
        value={text}
        onChange={(e) => {
          setText(e.target.value);
          const hex = parseHex(e.target.value);
          if (hex) onChange(hex);
        }}
        onBlur={() => setText(value)}
        onKeyDown={(e) => e.key === "Enter" && e.currentTarget.blur()}
        aria-invalid={parseHex(text) === null}
        aria-label={`${label}: hex code`}
        spellCheck={false}
        maxLength={7}
        className={cn("font-mono", stacked ? "h-7 w-20 px-1.5 text-xs md:text-xs" : "w-24")}
      />
    </div>
  );
}
