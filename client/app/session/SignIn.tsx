import {
  Band,
  Face,
  FaceWell,
  Faint,
  Field,
  Key,
  Lamp,
  Panel,
  Record,
  Records,
  Row,
  Spacer,
  Stack,
} from "@design/index";
import type { SignInBench } from "./useSignIn";

/**
 * Signing in.
 *
 * The gate. Everything behind it names the person who did it — D11's
 * non-repudiable floor is established here, not asserted later — so this is the
 * one screen where being unambiguous matters more than being quick.
 *
 * # One refusal for four different failures
 *
 * A wrong password, an unknown address, a locked account and a person marked
 * inactive all answer the same way, and that is the server's decision rather
 * than this screen's reticence: any distinction between them is an oracle for
 * whether somebody works here. The screen repeats what it was told and does not
 * elaborate.
 *
 * # The company question
 *
 * D19 makes a person global and membership per-tenant, so somebody who works
 * for two businesses must say which they are acting for. The server answers 409
 * with the list; this draws it. The page this replaces printed the error text
 * at somebody with no field to answer in, so that person could not sign in.
 *
 * # Two ways in, and the second is quieter
 *
 * A key is offered beside the password rather than instead of it, and it is not
 * the lit one: `Key` says two lit keys on a screen means neither is, and the
 * act this screen is for is still signing in. It needs no email — a
 * discoverable credential says whose it is — which the screen says beside it,
 * because a control offering to skip a field somebody is looking at should say
 * so before they fill it in.
 *
 * A browser with no `PublicKeyCredential` gets a sentence in place of the
 * control, the same way `/keys` does. Offering something that can only throw is
 * worse than saying it is not here.
 */
export function SignIn({ bench }: { bench: SignInBench }) {
  if (bench.state.kind === "done") {
    return (
      <Panel elevation="lifted" frame="bezel">
        <Face>
          <Row gap={3} wrap>
            <Lamp kind="recorded" />
            <span>Signed in…</span>
          </Row>
        </Face>
      </Panel>
    );
  }

  const choosing = bench.state.kind === "choose";
  // **Carrying on from the company question is a different act for each way
  // in.** One sends a password; the other starts a second ceremony, because the
  // first was spent answering the question. So the key that continues has a
  // different name and a different readiness: a passkey needs no password, and
  // requiring one here would leave somebody who has only a key on a screen
  // whose one control is disabled.
  const withKey = choosing && bench.via === "key";
  const ready = withKey
    ? bench.tenant !== null
    : bench.credentials.email.trim() !== "" &&
      bench.credentials.password !== "" &&
      (!choosing || bench.tenant !== null);
  const carryOn = () => (withKey ? void bench.useKey() : void bench.submit());

  return (
    <Panel elevation="lifted" frame="bezel" as="section">
      <Stack gap={3}>
        <Face pad={false}>
          <Band>Sign in</Band>
          <FaceWell>
            <Stack gap={4}>
              <Field
                label="Email"
                numeric={false}
                value={bench.credentials.email}
                onChange={(v) => bench.type("email", v)}
                disabled={bench.busy}
              />
              <Field
                label="Password"
                numeric={false}
                secret
                value={bench.credentials.password}
                onChange={(v) => bench.type("password", v)}
                disabled={bench.busy}
                onSubmit={() => ready && carryOn()}
              />
            </Stack>
          </FaceWell>
        </Face>

        {choosing && bench.state.kind === "choose" && (
          <Face pad={false}>
            <Band>Organisation</Band>
            <FaceWell>
              <Stack gap={3}>
                {/* Drawn as keys rather than a select: there are two or three
                    of these, they are the decision on the screen, and a
                    dropdown hides the answer behind a tap. */}
                <Records>
                  {bench.state.tenants.map((t) => (
                    <Record
                      key={t.tenant_id}
                      name={<span>{t.name}</span>}
                      action={
                        <Key
                          size="small"
                          live={bench.tenant === t.tenant_id}
                          onClick={() => bench.choose(t.tenant_id)}
                        >
                          {bench.tenant === t.tenant_id ? "Chosen" : "Choose"}
                        </Key>
                      }
                    />
                  ))}
                </Records>
              </Stack>
            </FaceWell>
          </Face>
        )}

        {bench.problem && (
          <Face>
            <Row gap={3} wrap>
              <Lamp kind="finding" />
              <span>{bench.problem}</span>
            </Row>
          </Face>
        )}

        <Face>
          <Row gap={3} align="center" wrap>
            {/* The key, and only while there is no company question standing:
                the answer to that carries on with whichever way in was being
                used, so a second control there would be a second way to answer
                one question. */}
            {!choosing &&
              (bench.keys ? (
                <>
                  <Key size="small" disabled={bench.busy} onClick={() => void bench.useKey()}>
                    Use a passkey
                  </Key>
                  <Faint>no email needed</Faint>
                </>
              ) : (
                // Said once, in place of a control that could never work.
                <Faint>This browser has no passkey support.</Faint>
              ))}
            <Spacer />
            <Key live={!choosing} disabled={bench.busy || !ready} onClick={carryOn}>
              {withKey
                ? "Use your key for this company"
                : choosing
                  ? "Sign in to this company"
                  : "Sign in"}
            </Key>
          </Row>
        </Face>
      </Stack>
    </Panel>
  );
}
