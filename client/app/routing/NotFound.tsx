import { Button, Card, Stack } from "@ui/index";

import { useNavigate } from "./Router";
import s from "@app/session/session-pages.module.css";

/**
 * A path this application does not have.
 *
 * The route table used to end `?? FixturePack`, so every typo and stale link
 * drew the pack screen full of invented data. Now it says so. The path is in
 * mono because it is a string somebody typed or followed (D114).
 */
export function NotFound({ path }: { path: string }) {
  const navigate = useNavigate();
  return (
    <Card>
      <Stack gap={5}>
        <div>
          <h1 className={s.title}>Page not found</h1>
          <p className={s.subtitle}>
            <code>{path}</code>
          </p>
          <p className={s.subtitle}>Check the address and try again.</p>
        </div>
        <Button variant="primary" size="lg" block onClick={() => navigate("/")}>
          Go to dashboard
        </Button>
      </Stack>
    </Card>
  );
}
