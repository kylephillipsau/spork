import { ArrowRight, SearchX } from "lucide-react";

import { Alert, Card, EmptyState, Link, List, ListItem, Page, PageHeader, Skeleton, Stack } from "@ui/index";
import { href } from "@app/routing/location";

import { FOUND_GROUPS } from "./found";
import type { SearchDesk } from "./useSearchResults";
import s from "./search-page.module.css";

/**
 * Everything a search matches (D227): where Enter in the header's search lands
 * when it matches more than one thing, so a family's code shows its members
 * rather than opening the first of them. Grouped as the header groups them;
 * the items can be opened in the item list, to narrow or export.
 */
export function SearchPage({ desk }: { desk: SearchDesk }) {
  const q = desk.q.trim();
  const read = desk.read;
  return (
    <Page>
      <PageHeader title="Search" description={q ? `What matches “${q}”.` : "Type in the search at the top."} />
      {read.kind === "failed" ? (
        <Alert tone="danger">{read.message}</Alert>
      ) : read.kind === "loading" ? (
        <Card>
          <Stack gap={3}>
            <Skeleton width="40%" />
            <Skeleton width="70%" />
          </Stack>
        </Card>
      ) : read.value.length === 0 ? (
        <Card>
          <EmptyState icon={<SearchX />} title={q ? `Nothing matches “${q}”` : "Nothing searched for yet"} />
        </Card>
      ) : (
        <Stack gap={5}>
          {FOUND_GROUPS.map((g) => {
            const found = read.value.filter((r) => r.kind === g.kind);
            if (found.length === 0) return null;
            return (
              <Card
                key={g.kind}
                title={g.label}
                count={found.length}
                padded={false}
                actions={
                  g.kind === "item" ? (
                    <Link href={href(`/items?q=${encodeURIComponent(q)}`)} className={s.more}>
                      In the item list <ArrowRight aria-hidden />
                    </Link>
                  ) : undefined
                }
              >
                <List label={g.label}>
                  {found.map((r) => (
                    <ListItem
                      key={r.id}
                      lead={<span className={s.icon}>{g.icon}</span>}
                      title={
                        <Link href={href(r.path)} className={s.title}>
                          {r.title}
                        </Link>
                      }
                      description={r.detail || undefined}
                    />
                  ))}
                </List>
              </Card>
            );
          })}
        </Stack>
      )}
    </Page>
  );
}
