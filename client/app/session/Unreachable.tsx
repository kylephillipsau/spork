import { useState } from "react";

import { Alert, Button, Inline } from "@ui/index";
import { serverUrl } from "@app/platform/server";

import { useSessionBench } from "./SessionContext";
import { ServerField } from "./ServerField";
import s from "./session-pages.module.css";

/**
 * The app's way back when its server can't be reached.
 *
 * A browser is always on the server that sent it, so there unreachable is
 * a passing state. The app keeps its sign-in on the device, and opens to its
 * screens with a server that is off or at another address: with no session
 * confirmed there is no account menu to sign out from, and the sign-in
 * screen's server field is never reached. So the app says so above every
 * screen, and offers the address and another try.
 */
export function Unreachable() {
  const { session, refresh } = useSessionBench();
  const [trying, setTrying] = useState(false);
  if (import.meta.env.MODE !== "mobile" || session.kind !== "unreachable") return null;
  return (
    <div className={s.unreachable}>
      <Alert tone="danger">
        Spork can’t be reached at {serverUrl().replace(/^https?:\/\//, "")}. Start it on that computer and try
        again, or use another address.
      </Alert>
      <Inline>
        <Button
          loading={trying}
          onClick={() => {
            setTrying(true);
            void refresh().finally(() => setTrying(false));
          }}
        >
          Try again
        </Button>
      </Inline>
      <ServerField unreachable />
    </div>
  );
}
