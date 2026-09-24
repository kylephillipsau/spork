import { useEffect, useState } from "react";
import { ChevronDown, Warehouse } from "lucide-react";

import { Menu, MenuLabel, MenuRadioGroup, cx, materials, useToast } from "@ui/index";
import { api, reason } from "@domain/api";
import type { SiteRow } from "@domain/types";
import { useSessionBench } from "@app/session/SessionContext";

import s from "./header.module.css";

/**
 * Which warehouse this session works at, and changing it (replaces /where as a
 * destination; D171). A woven nylon tag, dyed: the one label that says where
 * you are.
 */
export function SiteSwitcher() {
  const { session, refresh } = useSessionBench();
  const toast = useToast();
  const [sites, setSites] = useState<SiteRow[]>([]);

  const who = session.kind === "signed-in" ? session.who : null;

  useEffect(() => {
    if (!who) return;
    let live = true;
    api
      .sites()
      .then((rows) => {
        if (live) setSites(rows);
      })
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [who?.site_id, who]);

  if (!who) return null;

  const current = sites.find((x) => x.id === who.site_id);
  const label = who.site_code ?? "No site";

  async function choose(id: string) {
    if (id === who?.site_id) return;
    try {
      await api.chooseSite({ site_id: id });
      await refresh();
      // Everything on screen was read for the old site, so read it again.
      window.location.reload();
    } catch (e) {
      toast({ title: "Could not change site", description: reason(e, "Try again."), tone: "danger" });
    }
  }

  return (
    <Menu
      trigger={
        <button
          type="button"
          className={cx(s.site, materials.nylon, materials.dyed)}
          aria-label={`Warehouse: ${current?.name ?? label}. Change warehouse`}
        >
          <Warehouse className={s.siteIcon} aria-hidden />
          <span className={s.siteCode}>{label}</span>
          {sites.length > 1 && <ChevronDown className={s.siteChevron} aria-hidden />}
        </button>
      }
    >
      <MenuLabel>Warehouse</MenuLabel>
      <MenuRadioGroup
        value={who.site_id ?? ""}
        onValueChange={(v) => void choose(v)}
        options={sites.map((x) => ({ value: x.id, label: `${x.name} (${x.code})` }))}
      />
    </Menu>
  );
}
