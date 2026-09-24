import { useEffect, useState, type ReactNode } from "react";
import { Menu as MenuIcon, PanelLeftClose, PanelLeftOpen } from "lucide-react";

import { Breadcrumbs, IconButton, Link, Tooltip, cx, materials, type Crumb } from "@ui/index";
import { usePath } from "@app/routing/Router";
import { useWork } from "@app/nav/useWork";


import { NAV, SETTINGS, currentItem, type Badge, type NavGroup, type NavItem } from "./nav";
import { ScanSearch } from "./ScanSearch";
import { SiteSwitcher } from "./SiteSwitcher";
import { UserMenu } from "./UserMenu";
import s from "./app-shell.module.css";

const COLLAPSED_KEY = "spork.sidebar.collapsed";

function readCollapsed(): boolean {
  try {
    return localStorage.getItem(COLLAPSED_KEY) === "1";
  } catch {
    return false;
  }
}

/**
 * The desktop frame (D171): anodised sidebar, header, and the page.
 *
 * Below 900px the sidebar leaves the layout and opens over the page from the
 * header's menu button, and closes on navigation, Escape or a click outside.
 */
export function AppShell({
  screenId,
  title,
  showSearch = true,
  children,
}: {
  screenId: string;
  title: string;
  /** Off on screens that claim the scanner themselves (D117). */
  showSearch?: boolean | undefined;
  children: ReactNode;
}) {
  const path = usePath();
  const counts = useWork(screenId);
  const [collapsed, setCollapsed] = useState(readCollapsed);
  const [open, setOpen] = useState(false);

  useEffect(() => setOpen(false), [path]);
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open]);

  function toggleCollapsed() {
    setCollapsed((c) => {
      try {
        localStorage.setItem(COLLAPSED_KEY, c ? "0" : "1");
      } catch {
        // not remembered, still toggled
      }
      return !c;
    });
  }

  const here = currentItem(path);
  const crumbs: Crumb[] = [];
  if (here?.group.label) crumbs.push({ label: here.group.label });
  if (here && here.item.path !== "/") {
    crumbs.push({ label: here.item.label, href: here.item.path });
    if (path !== here.item.path) crumbs.push({ label: title });
  } else if (here) {
    crumbs.push({ label: here.item.label });
  } else {
    crumbs.push({ label: title });
  }

  return (
    <div className={cx(s.shell, collapsed && s.collapsed, open && s.open)}>
      <aside className={cx(s.sidebar, materials.anodise)} aria-label="Main">
        <div className={s.brand}>
          <Link variant="plain" href="/" className={s.brandLink}>
            <span className={cx(s.mark, materials.nylon, materials.dyed)} aria-hidden>
              S
            </span>
            <span className={s.wordmark}>Spork</span>
          </Link>
        </div>

        <nav className={s.nav}>
          {NAV.map((g, i) => (
            <Group key={g.label ?? i} group={g} here={here?.item.id} counts={counts} collapsed={collapsed} />
          ))}
        </nav>

        <div className={s.foot}>
          <Group group={SETTINGS} here={here?.item.id} counts={counts} collapsed={collapsed} />
          <button type="button" className={s.collapse} onClick={toggleCollapsed}>
            {collapsed ? <PanelLeftOpen /> : <PanelLeftClose />}
            <span className={s.itemLabel}>Collapse</span>
          </button>
        </div>
      </aside>

      <div className={s.scrim} onClick={() => setOpen(false)} aria-hidden />

      <div className={s.main}>
        <header className={s.header}>
          <span className={s.menuButton}>
            <IconButton label="Menu" icon={<MenuIcon />} onClick={() => setOpen(true)} />
          </span>
          <div className={s.crumbs}>
            <Breadcrumbs items={crumbs} />
          </div>
          <div className={s.tools}>
            {showSearch && <ScanSearch />}
            <SiteSwitcher />
            <UserMenu />
          </div>
        </header>
        <main className={s.content} id="main">
          {children}
        </main>
      </div>
    </div>
  );
}

function Group({
  group,
  here,
  counts,
  collapsed,
}: {
  group: NavGroup;
  here: string | undefined;
  counts: Readonly<Partial<Record<Badge, number>>>;
  collapsed: boolean;
}) {
  return (
    <div className={s.group}>
      {group.label && <p className={s.groupLabel}>{group.label}</p>}
      <ul className={s.items}>
        {group.items.map((item) => (
          <li key={item.id}>
            <Item item={item} current={here === item.id} count={item.badge ? counts[item.badge] : undefined} collapsed={collapsed} />
          </li>
        ))}
      </ul>
    </div>
  );
}

function Item({
  item,
  current,
  count,
  collapsed,
}: {
  item: NavItem;
  current: boolean;
  count: number | undefined;
  collapsed: boolean;
}) {
  const Icon = item.icon;
  const link = (
    <Link
      variant="plain"
      href={item.path}
      className={cx(s.item, current && s.current, current && materials.raised)}
      aria-current={current ? "page" : undefined}
    >
      <Icon className={s.itemIcon} aria-hidden />
      <span className={s.itemLabel}>{item.label}</span>
      {count ? (
        <span className={cx(s.itemCount, materials.nylon)} aria-label={`${count} waiting`}>
          {count > 99 ? "99+" : count}
        </span>
      ) : null}
    </Link>
  );
  return collapsed ? (
    <Tooltip content={count ? `${item.label} · ${count}` : item.label} side="right">
      {link}
    </Tooltip>
  ) : (
    link
  );
}
