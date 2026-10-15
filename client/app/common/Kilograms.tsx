import { useState } from "react";

import { TextField } from "@ui/index";

/** A weight in kilograms as typed, saved when it changes and makes sense. */
export function Kilograms({
  label,
  grams,
  placeholder,
  disabled,
  save,
  nothing = false,
}: {
  label: string;
  grams: number | null;
  placeholder: string;
  disabled: boolean;
  save: (grams: number | null) => void;
  /** Nought is an answer: a lid that carries nothing (D249). */
  nothing?: boolean;
}) {
  const said = grams === null ? "" : String(grams / 1000);
  const [typed, setTyped] = useState(said);
  const commit = () => {
    const t = typed.trim();
    if (t === said) return;
    const kg = Number(t);
    if (t !== "" && !(kg > 0 || (nothing && kg === 0))) {
      setTyped(said);
      return;
    }
    save(t === "" ? null : Math.round(kg * 1000));
  };
  return (
    <TextField
      aria-label={label}
      inputMode="decimal"
      placeholder={placeholder}
      trailing="kg"
      value={typed}
      disabled={disabled}
      onChange={(e) => setTyped(e.target.value)}
      onBlur={commit}
      onKeyDown={(e) => {
        if (e.key === "Enter") commit();
      }}
    />
  );
}
