import { Code, Face, Faint, Lamp, Link, Panel, Row, Stack } from "@design/index";
import { href } from "./location";

/**
 * A path this application does not have.
 *
 * **What this replaces is the worst thing in the client.** The route table
 * ended `?? FixturePack`, so every typo, every stale link and every unmatched
 * path rendered the pack screen filled with invented data — a screen lying
 * about which screen it was, and the single biggest reason the whole thing
 * read as a demo.
 *
 * The path is drawn in mono because it is a string somebody typed or a link
 * somebody followed, and D114 says a string a person reads back is monospace.
 */
export function NotFound({ path }: { path: string }) {
  return (
    <Panel elevation="raised" frame="bezel">
      <Stack gap={3}>
        <Face>
          <Row gap={3} wrap>
            <Lamp kind="finding" />
            <span>There is no screen at</span>
            <Code>{path}</Code>
          </Row>
        </Face>
        <Face>
          <Stack gap={3}>
            <Faint>The address is wrong, or the screen has moved.</Faint>
            <Row gap={3}>
              <Link href={href("/")}>Back to what is waiting</Link>
            </Row>
          </Stack>
        </Face>
      </Stack>
    </Panel>
  );
}
