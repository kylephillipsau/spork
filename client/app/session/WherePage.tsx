import { ChevronRight, CircleAlert, Warehouse } from "lucide-react";

import { Button, Card, EmptyState, Spinner, Stack } from "@ui/index";
import { api } from "@domain/api";
import { href } from "@app/routing/location";

import type { WhereBench } from "./useWhere";
import s from "./session-pages.module.css";

/**
 * Choosing a warehouse after sign-in, when there is more than one (D171
 * layout; logic in useWhere). With one, it is chosen without asking. Changing
 * it later is the header's site switcher.
 */
export function WherePage({ bench }: { bench: WhereBench }) {
  const st = bench.state;

  if (st.kind === "asking" || st.kind === "settled") {
    return (
      <Card>
        <div className={s.centre}>
          <Spinner size={20} label="Loading warehouses" />
        </div>
      </Card>
    );
  }

  if (st.kind === "nowhere" || st.kind === "failed") {
    return (
      <Card>
        <EmptyState
          icon={<Warehouse />}
          title={st.kind === "nowhere" ? "No warehouse assigned" : "Could not load warehouses"}
          description={
            st.kind === "nowhere"
              ? "Your account is not attached to a warehouse yet. Ask an administrator to add you to one."
              : st.message
          }
          action={
            <Button onClick={() => void api.signOff().finally(() => window.location.assign(href("/sign-in")))}>
              Sign out
            </Button>
          }
        />
      </Card>
    );
  }

  return (
    <Card>
      <Stack gap={5}>
        <div>
          <h1 className={s.title}>Select a warehouse</h1>
          <p className={s.subtitle}>You can change it later from the header.</p>
        </div>
        <ul className={s.sites}>
          {st.sites.map((site) => (
            <li key={site.id}>
              <button
                type="button"
                className={s.site}
                disabled={bench.busy}
                onClick={() => void bench.choose(site.id)}
              >
                <span className={s.siteCode}>{site.code}</span>
                <span className={s.siteName}>{site.name}</span>
                <ChevronRight className={s.siteGo} aria-hidden />
              </button>
            </li>
          ))}
        </ul>
        {bench.problem && (
          <p className={s.problem} role="alert">
            <CircleAlert aria-hidden />
            {bench.problem}
          </p>
        )}
      </Stack>
    </Card>
  );
}
