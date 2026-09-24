import { useState } from "react";
import {
  Download,
  Inbox,
  LogOut,
  Monitor,
  Moon,
  MoreHorizontal,
  Package,
  Plus,
  Settings,
  Sun,
  Trash2,
  User,
} from "lucide-react";

import {
  Avatar,
  Badge,
  Breadcrumbs,
  Button,
  Card,
  Checkbox,
  Count,
  DataTable,
  Dialog,
  Drawer,
  EmptyState,
  IconButton,
  Inline,
  Kbd,
  Link,
  Menu,
  MenuItem,
  MenuLabel,
  MenuRadioGroup,
  MenuSeparator,
  PageHeader,
  SearchField,
  Select,
  Skeleton,
  Spinner,
  Stack,
  Tabs,
  TextField,
  UiRoot,
  useTheme,
  useToast,
  type Column,
  type ThemePreference,
} from "@ui/index";

import s from "./ui-kit.module.css";

/**
 * The UI kit, every component in its states (review build only, at /ui-kit).
 * What the redesign is reviewed against before screens move onto it.
 */
export function UiKit() {
  return (
    <UiRoot>
      <Gallery />
    </UiRoot>
  );
}

interface OrderRow {
  order: string;
  customer: string;
  lines: number;
  units: number;
  status: "Picking" | "Packing" | "Ready" | "Short";
  updated: string;
}

const ORDERS: OrderRow[] = [
  { order: "SO10482", customer: "Acme Foods : Dandenong", lines: 3, units: 42, status: "Picking", updated: "10:42" },
  { order: "SO10479", customer: "PMFresh Pty Ltd", lines: 12, units: 380, status: "Packing", updated: "10:31" },
  { order: "SO10477", customer: "Harbour Supplies", lines: 1, units: 4, status: "Ready", updated: "09:58" },
  { order: "SO10470", customer: "Coastal Catering Co", lines: 6, units: 90, status: "Short", updated: "09:12" },
];

const TONE = { Picking: "info", Packing: "accent", Ready: "success", Short: "warning" } as const;

const COLUMNS: Column<OrderRow>[] = [
  { key: "order", header: "Order", cell: (r) => r.order, sort: (r) => r.order, mono: true, width: "120px" },
  { key: "customer", header: "Customer", cell: (r) => r.customer, sort: (r) => r.customer },
  { key: "lines", header: "Lines", cell: (r) => r.lines, sort: (r) => r.lines, align: "right", width: "80px" },
  { key: "units", header: "Units", cell: (r) => r.units, sort: (r) => r.units, align: "right", width: "80px" },
  {
    key: "status",
    header: "Status",
    cell: (r) => (
      <Badge tone={TONE[r.status]} dot>
        {r.status}
      </Badge>
    ),
    sort: (r) => r.status,
    width: "130px",
  },
  { key: "updated", header: "Updated", cell: (r) => r.updated, sort: (r) => r.updated, align: "right", width: "100px" },
];

function Gallery() {
  const { preference, setPreference } = useTheme();
  const toast = useToast();
  const [tab, setTab] = useState("open");
  const [site, setSite] = useState("MEL");
  const [checked, setChecked] = useState(true);
  const [drawer, setDrawer] = useState<OrderRow | null>(null);

  return (
    <div className={s.page}>
      <Stack gap={8}>
        <Inline justify="between" align="center">
          <Stack gap={1}>
            <Breadcrumbs items={[{ label: "Review", href: "/ui-kit" }, { label: "UI kit" }]} />
            <h1 className={s.h1}>Spork UI kit</h1>
          </Stack>
          <Select
            aria-label="Theme"
            value={preference}
            onValueChange={(v) => setPreference(v as ThemePreference)}
            options={[
              { value: "light", label: "Light" },
              { value: "dark", label: "Dark" },
              { value: "system", label: "System" },
            ]}
            size="sm"
          />
        </Inline>

        <section className={s.section}>
          <h2 className={s.h2}>Page header</h2>
          <Card>
            <PageHeader
              title="Orders"
              description="Sales orders waiting to be picked, packed and despatched."
              actions={
                <>
                  <Button icon={<Download />}>Import</Button>
                  <Button variant="primary" icon={<Plus />}>
                    New order
                  </Button>
                </>
              }
            />
          </Card>
        </section>

        <section className={s.section}>
          <h2 className={s.h2}>Buttons</h2>
          <Inline gap={2}>
            <Button variant="primary">Primary</Button>
            <Button>Secondary</Button>
            <Button variant="ghost">Ghost</Button>
            <Button variant="danger" icon={<Trash2 />}>
              Delete
            </Button>
            <Button variant="primary" loading>
              Saving
            </Button>
            <Button disabled>Disabled</Button>
            <Button size="sm">Small</Button>
            <Button size="lg" variant="primary">
              Large
            </Button>
            <IconButton label="Settings" icon={<Settings />} />
            <IconButton label="More" icon={<MoreHorizontal />} variant="secondary" />
          </Inline>
        </section>

        <section className={s.section}>
          <h2 className={s.h2}>Badges and status</h2>
          <Inline gap={2}>
            <Badge>Neutral</Badge>
            <Badge tone="accent">Accent</Badge>
            <Badge tone="success" dot>
              Ready
            </Badge>
            <Badge tone="warning" dot>
              Short
            </Badge>
            <Badge tone="danger" dot>
              Failed
            </Badge>
            <Badge tone="info" dot>
              Picking
            </Badge>
            <Count value={4} />
            <Count value={12} tone="accent" />
            <Avatar name="Kyle Phillips" />
            <Kbd>Ctrl</Kbd>
            <Kbd>K</Kbd>
            <Spinner />
          </Inline>
        </section>

        <section className={s.section}>
          <h2 className={s.h2}>Forms</h2>
          <Card>
            <div className={s.formGrid}>
              <TextField label="Order number" placeholder="SO10482" hint="As it appears in NetSuite." />
              <TextField label="Weight" type="number" defaultValue="12.4" trailing="kg" />
              <TextField label="Email" defaultValue="not-an-email" error="Enter a valid email address." />
              <Select
                label="Warehouse"
                value={site}
                onValueChange={setSite}
                options={[
                  { value: "MEL", label: "Melbourne" },
                  { value: "SYD", label: "Sydney" },
                  { value: "BNE", label: "Brisbane" },
                ]}
              />
              <SearchField label="Search" placeholder="Search orders…" />
              <div className={s.checkCell}>
                <Checkbox label="Only show my work" checked={checked} onCheckedChange={setChecked} />
              </div>
            </div>
          </Card>
        </section>

        <section className={s.section}>
          <h2 className={s.h2}>Table</h2>
          <Card padded={false}>
            <div className={s.toolbar}>
              <Tabs
                value={tab}
                onValueChange={setTab}
                aria-label="Order status"
                items={[
                  { value: "open", label: "Open", count: 4 },
                  { value: "short", label: "Short", count: 1 },
                  { value: "done", label: "Despatched" },
                ]}
              />
            </div>
            <DataTable
              aria-label="Orders"
              columns={COLUMNS}
              rows={ORDERS}
              rowKey={(r) => r.order}
              onRowClick={(r) => setDrawer(r)}
              initialSort={{ key: "updated", direction: "desc" }}
            />
          </Card>
          <Inline gap={4} align="start">
            <div className={s.half}>
              <Card padded={false} title="Loading">
                <DataTable columns={COLUMNS.slice(0, 3)} rows={[]} rowKey={(r) => r.order} loading />
              </Card>
            </div>
            <div className={s.half}>
              <Card padded={false} title="Empty">
                <DataTable
                  columns={COLUMNS.slice(0, 3)}
                  rows={[]}
                  rowKey={(r) => r.order}
                  empty={
                    <EmptyState
                      icon={<Inbox />}
                      title="No orders waiting"
                      description="Orders sent from NetSuite appear here."
                    />
                  }
                />
              </Card>
            </div>
          </Inline>
        </section>

        <section className={s.section}>
          <h2 className={s.h2}>Overlays</h2>
          <Inline gap={2}>
            <Dialog
              trigger={<Button>Open dialog</Button>}
              title="Void carton?"
              description="The carton is removed from the consignment. This cannot be undone."
              footer={
                <>
                  <Button>Cancel</Button>
                  <Button variant="danger">Void carton</Button>
                </>
              }
            />
            <Menu trigger={<Button icon={<User />}>Account menu</Button>}>
              <MenuLabel>Kyle Phillips</MenuLabel>
              <MenuItem icon={<User />}>Account</MenuItem>
              <MenuItem icon={<Settings />} shortcut="Ctrl ,">
                Settings
              </MenuItem>
              <MenuSeparator />
              <MenuLabel>Theme</MenuLabel>
              <MenuRadioGroup
                value={preference}
                onValueChange={(v) => setPreference(v as ThemePreference)}
                options={[
                  { value: "light", label: "Light", icon: <Sun /> },
                  { value: "dark", label: "Dark", icon: <Moon /> },
                  { value: "system", label: "System", icon: <Monitor /> },
                ]}
              />
              <MenuSeparator />
              <MenuItem icon={<LogOut />} danger>
                Sign out
              </MenuItem>
            </Menu>
            <Button
              onClick={() =>
                toast({ title: "Order sent to Spork", description: "SO10482 · 3 lines, 42 to pick", tone: "success" })
              }
            >
              Show toast
            </Button>
            <Link href="/ui-kit">A text link</Link>
          </Inline>
          <Card title="Loading placeholders">
            <Stack gap={2}>
              <Skeleton width="40%" />
              <Skeleton width="85%" />
              <Skeleton width="65%" />
            </Stack>
          </Card>
        </section>
      </Stack>

      <Drawer
        open={drawer !== null}
        onOpenChange={(o) => {
          if (!o) setDrawer(null);
        }}
        title={drawer?.order ?? ""}
        description={drawer?.customer}
        footer={
          <>
            <Button onClick={() => setDrawer(null)}>Close</Button>
            <Button variant="primary" icon={<Package />}>
              Start packing
            </Button>
          </>
        }
      >
        {drawer && (
          <Stack gap={3}>
            <Inline gap={2}>
              <Badge tone={TONE[drawer.status]} dot>
                {drawer.status}
              </Badge>
              <span className={s.muted}>Updated {drawer.updated}</span>
            </Inline>
            <p>
              {drawer.lines} lines, {drawer.units} units to pick.
            </p>
          </Stack>
        )}
      </Drawer>
    </div>
  );
}
