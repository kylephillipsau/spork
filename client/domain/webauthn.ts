/**
 * The base64url edge of a passkey ceremony.
 *
 * WebAuthn speaks `ArrayBuffer` and JSON does not, so every ceremony is the
 * same four lines of transcoding in both directions. They lived twice in
 * `web/mod.rs`, inlined into two maud pages as a string of JavaScript — which
 * is two copies with no test between them, of the one part of the flow where a
 * wrong answer looks like a rejected key rather than like a bug.
 *
 * Pure, and tested without a browser: `atob` and `btoa` are on Node as well.
 *
 * # Why base64url and not base64
 *
 * A credential id travels in JSON and sometimes in a URL, and `+` and `/` mean
 * something in both. The WebAuthn registries specify base64url, unpadded — so
 * `-` and `_` replace them and the `=` tail is dropped.
 */

/**
 * Whether this browser can run a ceremony at all.
 *
 * **A state, not a failure.** A browser with no `PublicKeyCredential` will
 * never enrol a key and will never sign in with one, so the screens say so once
 * instead of offering a control that always throws.
 */
export function supportsPasskeys(): boolean {
  return typeof window !== "undefined" && !!window.PublicKeyCredential;
}

/**
 * Whether the browser is telling us the operator said no.
 *
 * **Declining is not an error and must not be drawn as one.** Dismissing the
 * platform sheet arrives as an exception, the same way a real failure does, and
 * the maud pages caught both into one paragraph — so cancelling out of the
 * sheet left "Could not start." on the screen. Here rather than beside either
 * ceremony because both of them ask the question and the answer is the same.
 */
export function declined(error: unknown): boolean {
  if (!(error instanceof Error)) return false;
  return error.name === "NotAllowedError" || error.name === "AbortError";
}

/** `b64url` → bytes, restoring the padding the encoder dropped. */
export function b64urlToBytes(s: string): Uint8Array {
  const plain = s.replace(/-/g, "+").replace(/_/g, "/");
  // `%4` rather than a fixed count: the tail is 0, 2 or 3 characters short and
  // `atob` refuses anything that is not a multiple of four.
  const padded = plain + "=".repeat((4 - (plain.length % 4)) % 4);
  const raw = atob(padded);
  return Uint8Array.from(raw, (c) => c.charCodeAt(0));
}

/** Bytes → `b64url`, unpadded. */
export function bytesToB64url(buf: ArrayBuffer | Uint8Array): string {
  const bytes = buf instanceof Uint8Array ? buf : new Uint8Array(buf);
  let raw = "";
  // A byte at a time rather than `String.fromCharCode(...bytes)`: spreading a
  // large attestation object into arguments overflows the call stack, and an
  // attestation object is exactly the kind of thing that is occasionally large.
  for (const b of bytes) raw += String.fromCharCode(b);
  return btoa(raw).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

/** What the server sends, before the buffers are decoded. */
export interface RegistrationOptions {
  publicKey: {
    challenge: string;
    user: { id: string; name: string; displayName: string };
    excludeCredentials?: { id: string; type: string }[];
    [key: string]: unknown;
  };
}

export interface AuthenticationOptions {
  publicKey: {
    challenge: string;
    allowCredentials?: { id: string; type: string }[];
    [key: string]: unknown;
  };
}

/**
 * Decode a registration challenge in place of the strings the server sent.
 *
 * Returns the shape `navigator.credentials.create` wants. `excludeCredentials`
 * is what stops a second key being enrolled for a person who already has one on
 * this authenticator, so its ids are decoded too — forgetting them is a bug
 * that only shows up on the second enrolment from one device.
 */
export function decodeRegistration(options: RegistrationOptions): PublicKeyCredentialCreationOptions {
  const o = options.publicKey;
  return {
    ...o,
    challenge: b64urlToBytes(o.challenge),
    user: { ...o.user, id: b64urlToBytes(o.user.id) },
    excludeCredentials: (o.excludeCredentials ?? []).map((c) => ({
      ...c,
      id: b64urlToBytes(c.id),
      type: c.type as PublicKeyCredentialType,
    })),
  } as unknown as PublicKeyCredentialCreationOptions;
}

/** The same, for an assertion. */
export function decodeAuthentication(
  options: AuthenticationOptions,
): PublicKeyCredentialRequestOptions {
  const o = options.publicKey;
  return {
    ...o,
    challenge: b64urlToBytes(o.challenge),
    allowCredentials: (o.allowCredentials ?? []).map((c) => ({
      ...c,
      id: b64urlToBytes(c.id),
      type: c.type as PublicKeyCredentialType,
    })),
  } as unknown as PublicKeyCredentialRequestOptions;
}

/** What the server wants back from `navigator.credentials.create`. */
export function encodeAttestation(cred: PublicKeyCredential) {
  const response = cred.response as AuthenticatorAttestationResponse;
  return {
    id: cred.id,
    rawId: bytesToB64url(cred.rawId),
    type: cred.type,
    response: {
      attestationObject: bytesToB64url(response.attestationObject),
      clientDataJSON: bytesToB64url(response.clientDataJSON),
    },
    extensions: {},
  };
}

/** And from `navigator.credentials.get`. */
export function encodeAssertion(cred: PublicKeyCredential) {
  const response = cred.response as AuthenticatorAssertionResponse;
  return {
    id: cred.id,
    rawId: bytesToB64url(cred.rawId),
    type: cred.type,
    response: {
      authenticatorData: bytesToB64url(response.authenticatorData),
      clientDataJSON: bytesToB64url(response.clientDataJSON),
      signature: bytesToB64url(response.signature),
      // **Null rather than absent when there is none.** The user handle is how
      // a discoverable credential says whose it is, and a key enrolled with an
      // allow-list carries none — which is a different thing from a key that
      // forgot to send one.
      userHandle: response.userHandle ? bytesToB64url(response.userHandle) : null,
    },
    extensions: {},
  };
}
