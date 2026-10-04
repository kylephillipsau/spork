import { useState } from "react";
import { Server } from "lucide-react";

import { Button, Stack, TextField } from "@ui/index";
import { forgetSession } from "@domain/api";
import { serverSaid, serverUrl, setServer, tidyServer } from "@app/platform/server";

import s from "./session-pages.module.css";

/**
 * Which Spork the app signs in to, on the app's sign-in screen only: a
 * browser is always on the server that sent it. Open by itself the first time,
 * and whenever the server couldn't be reached, which is when the address is
 * the likeliest thing wrong.
 */
export function ServerField({ unreachable }: { unreachable: boolean }) {
  const [open, setOpen] = useState(() => !serverSaid());
  const [typed, setTyped] = useState(() => (serverSaid() ? serverUrl() : ""));
  const tidy = tidyServer(typed);
  const showing = open || unreachable;

  if (!showing) {
    return (
      <p className={s.server}>
        <Server aria-hidden />
        <span className={s.serverAt}>{serverUrl().replace(/^https?:\/\//, "")}</span>
        <Button size="sm" variant="ghost" onClick={() => setOpen(true)}>
          Change
        </Button>
      </p>
    );
  }
  return (
    <Stack gap={2}>
      <TextField
        label="Spork server"
        inputMode="url"
        autoCapitalize="off"
        autoCorrect="off"
        spellCheck={false}
        placeholder="192.168.1.20:8080"
        value={typed}
        onChange={(e) => setTyped(e.target.value)}
        hint="The address of the computer running Spork, as it shows on that computer: on the warehouse wifi or over Tailscale."
        error={typed.trim() && !tidy ? "That isn't an address" : undefined}
      />
      <Button disabled={!tidy || (serverSaid() && tidy === serverUrl())} onClick={() => tidy && setServer(tidy, forgetSession)}>
        Use this server
      </Button>
    </Stack>
  );
}
