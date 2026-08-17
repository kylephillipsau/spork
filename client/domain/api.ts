import type { Act } from "./acts";
import type { AuthenticationOptions, RegistrationOptions } from "./webauthn";
import type {
  ApiToken,
  CeremonyBegun,
  BenchScreen,
  BoundBarcode,
  CaptureScreen,
  ChangePasswordRequest,
  MintTokenRequest,
  MintedApiToken,
  Passkey,
  Workspace,
  ImportReport,
  ItemImportReport,
  ChooseSiteRequest,
  CurrentSession,
  ConsignmentResponse,
  DespatchScreen,
  DiscrepancyRow,
  FindingActionResponse,
  PasswordChanged,
  OrderMatch,
  PackJob,
  SignOnRequest,
  SignOnResponse,
  SiteRow,
  WorkWaiting,
  PickListScreen,
  PutawayScreen,
  ReceivingScreen,
  RecordReceiptResponse,
  RecordPickResponse,
  RecordEvidenceResponse,
  RecordObservationResponse,
  Resolution,
  SetupDone,
  SetupRequest,
  SetupStatus,
  ToWeigh,
  WeighingRecorded,
  Uuid,
} from "./types";

/**
 * The JSON API, as the client sees it.
 *
 * **Every ledger write takes an `act`**, and the caller supplies it rather than
 * this file minting one per invocation. See `acts.ts`: the id used to be minted
 * inside each body, so a retry after a lost response arrived wearing a
 * different name and the server recorded a second act. What a signature demands
 * cannot be forgotten; what a body mints quietly cannot be got right.
 *
 * **Every write carries a `client_event_id` the client mints.** The ledger is
 * append-only and idempotent by design: sending the same act twice changes
 * nothing, and acts may arrive in any order. That is what turns a network
 * dropout into a few late arrivals rather than into an offline mode with its
 * own code to maintain — so the identifier is generated here, before the
 * request, and reused if it has to be sent again.
 */

export class ApiError extends Error {
  constructor(
    message: string,
    readonly status: number,
    /**
     * What the server answered, parsed, when it answered JSON.
     *
     * **Kept because some refusals carry a question.** `POST /sessions` answers
     * 409 with the list of companies a person works for, and a client that read
     * only the message would have nothing to draw the choice from — which is
     * exactly why the page this replaces locked out anybody with two employers.
     * `unknown`, so a caller has to narrow it rather than trust its shape.
     */
    readonly body: unknown = null,
  ) {
    super(message);
    this.name = "ApiError";
  }
}

/**
 * What to tell the operator when something did not work.
 *
 * **The server's own words, or ours — never the browser's.** An `ApiError`
 * carries a sentence written on the write path for a person to read, and that
 * is the one to show. Anything else is a network fault or a bug, and the
 * `fetch` failure for those says "Load failed" or "NetworkError when attempting
 * to fetch resource", which tells an operator nothing and looks like the system
 * blaming them for its own plumbing. So the caller supplies a fallback that
 * names what did not happen.
 *
 * Written out by hand at eighteen call sites before this, and one of them had
 * drifted: `usePicking` narrowed on `Error` rather than `ApiError`, so a pick
 * list that could not reach the server said "Failed to fetch".
 */
export function reason(error: unknown, fallback: string): string {
  return error instanceof ApiError ? error.message : fallback;
}


/**
 * Where the JSON API answers.
 *
 * **Applied here and nowhere else**, which is why the ~40 endpoint definitions
 * below still read as the paths they are — `/fulfilments/{id}/bench`, not
 * `/api/fulfilments/{id}/bench`. A prefix repeated at forty call sites is forty
 * chances to leave one behind.
 *
 * The API has a prefix so the *client* can have the root: every screen wants a
 * name a person could read down a phone, and `/capture` and `/setup` were
 * already endpoints.
 */
const API = "/api";

/**
 * How this client reaches the server, which is not the same on every device.
 *
 * **One seam, and the web is the default so nothing changes by existing.** In a
 * browser the API is same-origin and the cookie is the credential; in the Tauri
 * webview the origin is `tauri://` and the API is somewhere else entirely, so
 * every request is cross-origin, goes through the native HTTP client, and
 * carries the bearer token `POST /sessions` has always returned — *"for clients
 * that cannot hold a cookie — D5's handhelds"*, which is this one, four months
 * after the sentence was written.
 *
 * Modelled on `onUnauthorised` below rather than invented: a policy the whole
 * file needs, registered once by whoever knows the platform, with a default
 * that is the behaviour there has always been.
 */
export interface Transport {
  /** Absolute on a device, `/api` in a browser. No trailing slash. */
  base: string;
  /** The native client under Tauri; the webview's own otherwise. */
  fetch: typeof globalThis.fetch;
  /** Sent with every request. Empty in a browser, where the cookie carries it. */
  headers: () => Record<string, string>;
  /** `same-origin` in a browser; `omit` where there is no cookie to send. */
  credentials: RequestCredentials;
  /** Hold on to what sign-on returned, or forget it. A no-op in a browser. */
  remember: (token: string | null) => void;
}

const WEB: Transport = {
  base: API,
  // Bound rather than referenced: `fetch` throws `Illegal invocation` if it is
  // called with anything but `globalThis` as its receiver, and storing it on an
  // object does exactly that.
  fetch: (...args) => globalThis.fetch(...args),
  headers: () => ({}),
  credentials: "same-origin",
  remember: () => {},
};

let transport: Transport = WEB;

/**
 * Point this client at a server it does not share an origin with.
 *
 * Called once, by the platform host, before anything fetches. Nothing in
 * `app/` or `design/` imports it: the screens do not know there is more than
 * one way to reach a server, which is the point of the seam.
 */
export function useTransport(next: Transport): void {
  transport = next;
}

/**
 * Where a content-addressed photograph's bytes are (D132).
 *
 * Exported because `Photo` draws pictures and does not know an API exists —
 * nothing under `design/` imports from here, deliberately, so the URL is built
 * on this side and passed in.
 */
export const imageUrl = (digest: string): string => `${transport.base}/images/${digest}`;

/**
 * What to do when the server says nobody is signed in.
 *
 * **One seam, registered once by the session provider.** A session expiring
 * mid-shift surfaces on whatever call the operator happened to make next, and
 * the alternative to a seam here is every hook checking `error.status === 401`
 * for itself — nine copies of one policy, free to drift apart.
 *
 * The error is still thrown: this is a notification, not a substitute for the
 * caller handling its own failure.
 */
let unauthorised: (() => void) | null = null;
export function onUnauthorised(handler: () => void): void {
  unauthorised = handler;
}

/**
 * A request carrying a body that is not JSON and a credential that is not the
 * session's (D158).
 *
 * The import endpoints take the CSV verbatim and an import token, so neither
 * half of `send` fits: it would stringify the body and lean on the cookie.
 * Everything after the request is shared — the 401 hook, the `detail`-first
 * error reading — because that behaviour should not fork.
 */
async function sendRaw<T>(
  method: string,
  path: string,
  body: string,
  token: string,
): Promise<T> {
  // No `transport.headers()`: the credential here is the import token and not
  // the session's, and sending both would be sending two answers to one
  // question. The base and the client are still the platform's.
  const response = await transport.fetch(`${transport.base}${path}`, {
    method,
    headers: { "content-type": "text/csv", authorization: `Bearer ${token}` },
    body,
  });
  return unwrap<T>(response);
}

async function unwrap<T>(response: Response): Promise<T> {
  if (!response.ok) {
    if (response.status === 401) unauthorised?.();
    let detail = `That did not work (${response.status}).`;
    let body: unknown = null;
    try {
      body = await response.json();
      const parsed = body as { detail?: string; error?: string };
      detail = parsed.detail ?? parsed.error ?? detail;
    } catch {
      /* an empty body is its own answer */
    }
    throw new ApiError(detail, response.status, body);
  }
  if (response.status === 204) return undefined as T;
  return (await response.json()) as T;
}

async function send<T>(method: string, path: string, body?: unknown): Promise<T> {
  const response = await transport.fetch(`${transport.base}${path}`, {
    method,
    credentials: transport.credentials,
    headers: {
      ...transport.headers(),
      ...(body === undefined ? {} : { "content-type": "application/json" }),
    },
    ...(body === undefined ? {} : { body: JSON.stringify(body) }),
  });

  // `ApiError` on the server puts the class in `error` and the reasoning in
  // `detail`. `unwrap` reads the reasoning if it is there — a handler that took
  // the trouble to write a sentence should not be reported as a number.
  return unwrap<T>(response);
}

export const api = {
  bench: (fulfilment: Uuid) => send<BenchScreen>("GET", `/fulfilments/${fulfilment}/bench`),

  /** A carton comes into existence somewhere. D97: `created` asserts placement,
   *  which is why the dock is part of the act rather than a later move. */
  startCarton: (input: { fulfilment: Uuid; preset: Uuid; dock: Uuid; act: Act }) =>
    send<unknown>("POST", "/packages", {
      // Two client-minted ids and both are retry keys: the package's own, and
      // the event's. A second press that minted either afresh would be a
      // second carton.
      id: input.act.id("package"),
      fulfilment_id: input.fulfilment,
      package_type_id: input.preset,
      location_id: input.dock,
      client_event_id: input.act.id("event"),
      occurred_at: input.act.at,
    }),

  /**
   * Claim, then record. **A pick with no claim behind it drives picked past
   * covered, which J56 raises as a finding** — so the allocation is not a
   * formality, it is what stops the bench seeding findings for everyone else.
   */
  async pickInto(input: {
    line: Uuid;
    stock: Uuid;
    carton: Uuid;
    quantity: number;
    act: Act;
  }): Promise<void> {
    // **Two calls, one act, and the allocation's `id` is a retry key too** —
    // Q174: *"replay returns the prior claim, a reused id with a different body
    // is refused"*. A second press behind a lost response used to mint a second
    // claim, which drives `covered` past what was asked and raises J56 as a
    // finding on somebody else's screen.
    await send<unknown>("POST", "/allocations", {
      id: input.act.id("allocation"),
      fulfilment_line_id: input.line,
      stock_id: input.stock,
      quantity: input.quantity,
    });
    await send<unknown>("POST", "/picks", {
      from_stock_id: input.stock,
      to_package_id: input.carton,
      fulfilment_line_id: input.line,
      quantity: input.quantity,
      client_event_id: input.act.id("event"),
      occurred_at: input.act.at,
    });
  },

  /**
   * **Two events, because they were come by two ways.** The weight is read off
   * a scale; a cut-down height is known because the packer cut the box.
   * `method` is the column that says which, and one event cannot say both.
   *
   * Nothing is sent for a height that still equals the preset's: seeding a
   * field is not recording a measurement, and a value nobody changed was never
   * observed.
   */
  async measure(input: {
    carton: Uuid;
    weightKg?: string;
    heightMm?: string;
    act: Act;
  }): Promise<void> {
    // One press, two acts, two names — see `acts.ts`. Naming them is what lets
    // a retry replay each half independently: a press whose weight landed and
    // whose height did not sends the weight again as a replay and the height
    // as the first attempt it is.
    if (input.weightKg) {
      await send<unknown>("POST", "/observations", {
        package_id: input.carton,
        measurements: [{ metric: "gross_weight", entered_value: input.weightKg, unit: "kg" }],
        method: "instrument",
        ingestion_channel: "scale",
        client_event_id: input.act.id("weight"),
        occurred_at: input.act.at,
      });
    }
    if (input.heightMm) {
      await send<unknown>("POST", "/observations", {
        package_id: input.carton,
        measurements: [{ metric: "height", entered_value: input.heightMm, unit: "mm" }],
        method: "keyed",
        ingestion_channel: "keyed",
        client_event_id: input.act.id("height"),
        occurred_at: input.act.at,
      });
    }
  },

  /**
   * Reverse picks newest first until the asked amount is covered.
   *
   * **One movement per correction, because a correction names the movement it
   * reverses.** A row can be several picks, and treating it as one fails on the
   * second pick of the same item into the same carton.
   */
  async takeOut(input: {
    picks: [Uuid, number][];
    quantity: number;
    reason: Uuid;
    act: Act;
  }): Promise<void> {
    let left = input.quantity;
    for (const [movement, available] of input.picks) {
      if (left <= 0) break;
      const take = Math.min(left, available);
      await send<unknown>("POST", "/corrections", {
        reverses_movement_id: movement,
        quantity: take,
        adjustment_reason_id: input.reason,
        // Named by the movement it reverses, so a loop that stopped halfway
        // replays what it did and carries on from where it stopped.
        client_event_id: input.act.id(`correction:${movement}`),
      });
      left -= take;
    }
    if (left > 0) throw new ApiError("Only part of that could be taken back out.", 409);
  },

  despatchBench: (site: Uuid) => send<DespatchScreen>("GET", `/sites/${site}/despatch`),

  /**
   * Stage 6, without the pending consignment or the eyeball match.
   *
   * **The id is minted here**, because a consignment is a grouping and carries
   * no `client_event` — so the identifier is the only thing that makes a retry
   * a replay rather than a second booking. The server answers a replay with the
   * consignment it already has and says so.
   */
  consign: (input: {
    act: Act;
    packages: Uuid[];
    carrier?: Uuid;
    service?: Uuid;
    provider?: Uuid;
    despatchAt?: string;
  }) =>
    send<ConsignmentResponse>("POST", "/consignments", {
      // The doc above says it: *"the identifier is the only thing that makes a
      // retry a replay rather than a second booking."* Minted per invocation,
      // it made every retry a second booking.
      id: input.act.id("consignment"),
      package_ids: input.packages,
      ...(input.carrier ? { carrier_id: input.carrier } : {}),
      ...(input.service ? { carrier_service_id: input.service } : {}),
      ...(input.provider ? { freight_provider_id: input.provider } : {}),
      ...(input.despatchAt ? { despatch_at: input.despatchAt } : {}),
    }),

  /**
   * Stage 8, as a ledger entry rather than a status.
   *
   * A despatch is stock leaving, and stock moves by line — so a carton is
   * despatched by despatching everything in it. One act per line, each with its
   * own `client_event_id`, so a half-finished carton retries cleanly instead of
   * double-counting the lines that already went.
   */
  async despatchCarton(input: {
    carton: Uuid;
    lines: { fulfilment_line_id: Uuid; quantity: number }[];
    act: Act;
  }): Promise<void> {
    if (input.lines.length === 0) {
      throw new ApiError("That carton has nothing in it to despatch.", 409);
    }
    for (const line of input.lines) {
      await send<unknown>("POST", `/packages/${input.carton}/despatch`, {
        fulfilment_line_id: line.fulfilment_line_id,
        quantity: line.quantity,
        // Named by the carton *and* the line, which is what the doc above
        // already promised: *"a half-finished carton retries cleanly instead
        // of double-counting the lines that already went."* It did not, until
        // the name was stable. The carton is in the name because despatching
        // several under one act is one press — and a line split across two
        // cartons would otherwise be one name for two acts.
        client_event_id: input.act.id(`line:${input.carton}:${line.fulfilment_line_id}`),
        occurred_at: input.act.at,
      });
    }
  },

  /**
   * The findings queue (D8), filtered by state.
   *
   * `state` is comma-separated and the server defaults it to `open`. The screen
   * always sends one, because "the default" is a thing the screen would then be
   * unable to name in its own tabs.
   */
  findings: (states: string) =>
    send<DiscrepancyRow[]>("GET", `/discrepancies?state=${encodeURIComponent(states)}`),

  /**
   * One finding, by id, in whatever state it is in.
   *
   * **What makes a findings link a link (D135).** The queue read answers a set
   * of states; a link somebody was sent names a finding and says nothing about
   * which tab it belongs on, so the screen reads it directly and works out the
   * tab from what comes back.
   */
  finding: (id: Uuid) => send<DiscrepancyRow>("GET", `/discrepancies/${id}`),

  /** `open` → `investigating`. Idempotent, with a soft warning if it already
   *  was. Answers with the whole finding, so nothing re-reads to redraw a row. */
  investigate: (id: Uuid, note?: string) =>
    send<FindingActionResponse>("POST", `/discrepancies/${id}/investigate`, note ? { note } : {}),

  /**
   * Manager sign-off that the model stands, with no ledger write.
   *
   * **The reason is required and the server enforces it**, which is the point
   * rather than a validation: accepting a finding is saying the disagreement is
   * explained, and an acceptance with no explanation is the thing D8 exists to
   * stop happening quietly. Distinct from `/adjustments`, which writes a
   * world-event movement and resolves rather than accepts.
   */
  accept: (id: Uuid, reason: string) =>
    send<FindingActionResponse>("POST", `/discrepancies/${id}/accept`, { reason }),

  /**
   * Bind a scanned identifier to this item, at a packaging level (D164).
   *
   * **The write path that did not exist.** `item_barcode` has been readable
   * since migration 79 and nothing could write to it, so a scan resolved
   * through whatever an importer had loaded and through `item.code` otherwise.
   * The level is required and is the point: a carton's label and a box's label
   * both resolve to the same item and mean different quantities of it.
   *
   * A refusal names what the string already means, which is the only answer
   * that helps somebody holding it.
   */
  bindBarcode: (item: Uuid, input: { scan: string; packaging_level: string; quantity: number | null }) =>
    send<BoundBarcode>("POST", `/items/${item}/barcodes`, input),

  /** What this item answers to, newest first. */
  itemBarcodes: (item: Uuid) => send<BoundBarcode[]>("GET", `/items/${item}/barcodes`),

  /** What to put on the scale, in the order worth doing it. */
  toWeigh: (limit = 50) => send<ToWeigh[]>("GET", `/revalidation?limit=${limit}`),

  /**
   * Put a thing on the scale and record what it said.
   *
   * **Always `instrument` off a `scale`, and the endpoint decides that rather
   * than the caller.** A weighing that was actually somebody typing a number
   * they remembered is a different fact and goes through `/observations`, where
   * it can say so. That distinction is the whole reason an interval means
   * anything.
   *
   * The value is a string for the same reason every other measurement is: a
   * JSON number is an IEEE double and 12.1 is not one, so sending a float would
   * round the evidence before storing it.
   */
  weigh: (input: {
    item?: Uuid;
    style?: Uuid;
    level: string;
    entered: string;
    unit: string;
    act: Act;
  }) =>
    send<WeighingRecorded>("POST", "/weighings", {
      ...(input.item ? { item_id: input.item } : {}),
      ...(input.style ? { item_style_id: input.style } : {}),
      packaging_level: input.level,
      entered_value: input.entered,
      unit: input.unit,
      client_event_id: input.act.id("event"),
      occurred_at: input.act.at,
    }),

  /** Whether this deployment has anybody in it (D142). Unauthenticated: it is
   *  the one read that has to work before there is anyone to be. */
  setupStatus: () => send<SetupStatus>("GET", "/setup"),

  /** Create the first administrator. Once, and only with the token the server
   *  printed to its log. */
  setUp: (input: SetupRequest) => send<SetupDone>("POST", "/setup", input),

  /**
   * Sign on.
   *
   * **The one call that answers before a tenant is known.** A person who works
   * for two businesses must say which they are acting for (D19), and the server
   * answers 409 with the list rather than choosing — so a caller has to be
   * ready for a question rather than only for a yes or a no.
   */
  /**
   * **The token is handed to the transport, not just returned.** A browser
   * ignores it and uses the cookie; a device has no cookie and this is the only
   * moment the token exists. Doing it here rather than in `useSignIn` means the
   * passkey path cannot forget to — both ways in reach one line.
   */
  async signOn(input: SignOnRequest): Promise<SignOnResponse> {
    const session = await send<SignOnResponse>("POST", "/sessions", input);
    transport.remember(session.token);
    return session;
  },

  /** Sign off. Revokes the record, so the token is dead everywhere at once —
   *  which is the thing a server-side session buys over a signed claim. */
  /** And forgotten on the way out, for the same reason it is kept on the way
   *  in: on a device the token *is* the session, so a sign-out that left it in
   *  place would revoke the session server-side and keep the key to it. */
  async signOff(): Promise<{ signed_off: boolean }> {
    try {
      return await send<{ signed_off: boolean }>("DELETE", "/sessions/current");
    } finally {
      transport.remember(null);
    }
  },

  /** Whose session this is. The chrome needs a name and a site, and a screen
   *  about a credential needs to say plainly whose credential it is. */
  currentSession: () => send<CurrentSession>("GET", "/sessions/current"),

  /** What there is to pack, at the site the session names. The grouping comes
   *  back on each job, computed by the server so one rule serves every screen. */
  packingQueue: (q: string) =>
    send<PackJob[]>("GET", `/packing${q.trim() ? `?q=${encodeURIComponent(q.trim())}` : ""}`),

  /**
   * Find an order by the number a customer quotes (D39, D44).
   *
   * One reference can name a cancelled order and the one that replaced it, so
   * the answer is a list even when it feels like a lookup.
   */
  findOrders: (reference: string) =>
    send<OrderMatch[]>("GET", `/orders?reference=${encodeURIComponent(reference)}`),

  /** The latest orders at this site, for somebody who has not been given a
   *  number to type. A way in rather than a report, so it is a fixed handful. */
  latestOrders: () => send<OrderMatch[]>("GET", "/orders"),

  /** What is waiting for you, at this site, now (D112). One read, shared by
   *  the landing screen and the rail's badges so they cannot disagree. */
  work: () => send<WorkWaiting>("GET", "/work"),

  /** Where this caller could be working. Under row-level security, so it is
   *  this tenant's warehouses and nobody else's. */
  sites: () => send<SiteRow[]>("GET", "/sites"),

  /**
   * Say where you are working.
   *
   * **This replaces the session rather than editing it** — `site_id` means
   * "where they signed on", and a row mutated under a live token would make one
   * session name two places. So the answer carries a new cookie and the old
   * session is revoked.
   */
  chooseSite: (input: ChooseSiteRequest) =>
    send<CurrentSession>("POST", "/sessions/site", input),

  /**
   * Change your own password, and end every other session (D143).
   *
   * **The current password goes up even though a session is already open.**
   * That is not belt-and-braces: it is the difference between a stolen session,
   * which expires, and a stolen account, which does not. The server requires it
   * and counts a wrong one against the same lockout a sign-on uses.
   */
  changePassword: (input: ChangePasswordRequest) =>
    send<PasswordChanged>("POST", "/credentials/password", input),

  /**
   * Load a bin list (D158).
   *
   * **The token is passed rather than taken from the session**, because the
   * import endpoints accept one kind of credential and a session is not it.
   * The caller mints one, hands it here, and withdraws it afterwards.
   *
   * The body is the CSV itself: the export is the interface, and re-shaping it
   * into JSON would put a translation between the system of record and this.
   */
  importBins: (
    token: string,
    csv: string,
    q: { apply?: boolean; assumeKind?: string; includeExternal?: boolean },
  ) => {
    const search = new URLSearchParams();
    if (q.apply) search.set("apply", "true");
    if (q.assumeKind) search.set("assume_kind", q.assumeKind);
    if (q.includeExternal) search.set("include_external", "true");
    const query = search.toString();
    return sendRaw<ImportReport>(
      "POST",
      `/import/bins${query ? `?${query}` : ""}`,
      csv,
      token,
    );
  },

  /** Load the item master. Same credential rules as `importBins`. */
  importItems: (token: string, csv: string, q: { apply?: boolean }) =>
    sendRaw<ItemImportReport>(
      "POST",
      `/import/items${q.apply ? "?apply=true" : ""}`,
      csv,
      token,
    ),

  /** The organisation and its warehouses. */
  workspace: () => send<Workspace>("GET", "/workspace"),

  /** Every import token this tenant has, spent ones included (D158). */
  apiTokens: () => send<ApiToken[]>("GET", "/tokens"),

  /** Mint one. **The answer carries the secret and nothing else ever will.** */
  mintApiToken: (input: MintTokenRequest) =>
    send<MintedApiToken>("POST", "/tokens", input),

  revokeApiToken: (id: string) => send<void>("DELETE", `/tokens/${id}`),

  /** The keys that can become you, so you can see them and take one away. */
  passkeys: () => send<Passkey[]>("GET", "/passkeys"),

  beginPasskeyRegistration: (label: string | null) =>
    send<CeremonyBegun<RegistrationOptions>>("POST", "/passkeys/registration/begin", { label }),

  finishPasskeyRegistration: (body: unknown) =>
    send<{ id: string }>("POST", "/passkeys/registration/finish", body),

  revokePasskey: (id: string) => send<void>("DELETE", `/passkeys/${id}`),

  /**
   * Offer to sign in with a key.
   *
   * **The email is optional and absent is the better path.** Given, the server
   * offers the keys that person holds; omitted, it starts a discoverable
   * ceremony and learns who it is talking to from the assertion — which is one
   * fewer field to type on a touchscreen with gloves on. The server answers the
   * same shape either way, on purpose: an endpoint that says "no such person"
   * is an endpoint that enumerates people.
   *
   * Runs before a session exists, like `signOn` beside it.
   */
  beginPasskeyAuthentication: (email: string | null) =>
    send<CeremonyBegun<AuthenticationOptions>>(
      "POST",
      "/passkeys/authentication/begin",
      email ? { email } : {},
    ),

  /**
   * Hand back what the authenticator signed, and get a session for it.
   *
   * Answers what `POST /sessions` answers, including the 409 that lists the
   * companies somebody works for — the same question D19 makes the password
   * path ask, arriving at the same place.
   */
  async finishPasskeyAuthentication(body: unknown): Promise<SignOnResponse> {
    const session = await send<SignOnResponse>(
      "POST",
      "/passkeys/authentication/finish",
      body,
    );
    transport.remember(session.token);
    return session;
  },

  capture: () => send<CaptureScreen>("GET", "/capture"),

  /**
   * What the thing in your hand is (D111, D34).
   *
   * **The raw string goes up unmodified.** What the reader sent is evidence —
   * the symbology prefix, the FNC1 separators and all — and a client that trims
   * it has made a decision the server cannot see. The server parses, and
   * `encodeURIComponent` is what carries a `\x1D` through a query string
   * intact.
   *
   * `expect` narrows: a screen that can only act on items says so, and a
   * carton scanned there reports `identifier_unknown` rather than navigating
   * somewhere nobody asked to go.
   */
  resolve: (scan: string, expect?: string) =>
    send<Resolution>(
      "GET",
      `/resolve?scan=${encodeURIComponent(scan)}` +
        (expect ? `&expect=${encodeURIComponent(expect)}` : ""),
    ),

  /**
   * A capture session, as one act (D133).
   *
   * **Every figure in one request.** The endpoint takes a `Vec` of
   * measurements and mints one `observation_event` per call, so sending the
   * weight and then the dimensions would produce two events and split the
   * photographs between them — and D132's join, "these pictures and these
   * figures are the same look at the same box", would go on answering the
   * wrong question while every row involved looked correct.
   *
   * The cost is that nothing here is durable until this returns. That is
   * deliberate and it is the screen's job not to imply otherwise.
   *
   * `method` is `instrument` because a scale and a tape measure both are one;
   * `ingestion_channel` is `keyed` because the figure reached us by being
   * typed on the handheld rather than read off a connected device. The pack
   * bench says `scale` for exactly that reason and means something different
   * by it.
   */
  /**
   * What to pick at a site, in walking order, with a picture of each thing.
   *
   * The question `crate::picking_list` declined to answer — which carton a pick
   * goes into — is answered by [`pick`] below, and by the picker rather than by
   * this model: they scan what they are putting the goods on (D166).
   */
  picking: (site: Uuid) => send<PickListScreen>("GET", `/sites/${site}/picking`),

  /**
   * What is expected here and has not all arrived.
   *
   * Each line says what receiving it will need before the pallet is broken
   * down: whether the policy demands a lot, which packaging levels it can be
   * counted in, and — when the promise names no owner — who already owns this
   * item here.
   */
  receiving: (site: Uuid) => send<ReceivingScreen>("GET", `/sites/${site}/receiving`),

  /**
   * Record one line of a delivery.
   *
   * **`goods_receipt_id` is the truck.** D43 and Q172: one delivery is one
   * header, and lines join by carrying the same client-minted id. The first
   * line of a session mints it and every line after it repeats it, which is
   * what makes a five-line drop one delivery rather than five.
   *
   * **The lot arrives as a code, not an id.** Nothing creates a `lot` row and
   * no screen could hand one over, so an item under a lot-requiring policy
   * could not be received at all. `lot_code` is found or created against
   * `UNIQUE (tenant_id, item_id, code)`, which is what makes the retry safe.
   * `lot_expiry` fills a blank and never overwrites — a disagreement comes back
   * in `warnings` rather than being settled by whoever received last.
   *
   * **A refusal is an answer.** `accepted: false` with a `discrepancy_id` is
   * what a line missing a required lot returns, and it is not an error: the
   * goods are on the dock either way and the screen says which happened.
   */
  receive: (input: {
    supply: Uuid;
    to: Uuid;
    delivery: Uuid;
    entered: number;
    level: string;
    config?: Uuid | null;
    owner?: Uuid | null;
    lotCode?: string | null;
    lotExpiry?: string | null;
    closePromise?: boolean;
    act: Act;
  }) =>
    send<RecordReceiptResponse>("POST", "/receipts", {
      expected_supply_id: input.supply,
      to_location_id: input.to,
      goods_receipt_id: input.delivery,
      entered_quantity: input.entered,
      entered_packaging_level: input.level,
      ...(input.config ? { item_packing_config_id: input.config } : {}),
      ...(input.owner ? { owner_id: input.owner } : {}),
      ...(input.lotCode ? { lot_code: input.lotCode } : {}),
      ...(input.lotExpiry ? { lot_expiry: input.lotExpiry } : {}),
      ...(input.closePromise ? { close_promise: true } : {}),
      client_event_id: input.act.id("event"),
      occurred_at: input.act.at,
    }),

  /**
   * What is on the dock with nowhere to live yet.
   *
   * Empty on a floor that receives straight to the shelf, which `POST /receipts`
   * has always allowed — and that is the right answer there, not a fault.
   */
  putaway: (site: Uuid) => send<PutawayScreen>("GET", `/sites/${site}/putaway`),

  /**
   * Put it away: one ledger row, from the dock cell to the bin.
   *
   * **`reason` follows the source, not the screen.** From a dock it is a
   * put-away; from storage the same act is a move. That is a fact about what
   * happened rather than a judgement about it, which is the line D114 draws —
   * and `stock_movement.reason` folds nothing either way (D99/D100 fold by
   * shape), so it is vocabulary for the person reading the ledger later.
   *
   * No claim to make first, unlike a pick: nothing is covered by putting goods
   * on a shelf, so J56 has nothing to say and `POST /allocations` is not called.
   */
  putAway: (input: {
    stock: Uuid;
    to: Uuid;
    quantity: number;
    reason: "putaway" | "move";
    act: Act;
  }) =>
    send<unknown>("POST", "/moves", {
      from_stock_id: input.stock,
      to_location_id: input.to,
      quantity: input.quantity,
      reason: input.reason,
      client_event_id: input.act.id("event"),
      occurred_at: input.act.at,
    }),

  /**
   * Take goods off a shelf and put them somewhere (D166).
   *
   * **Claim first, and only as much as is missing.** A pick with no claim
   * behind it drives `picked` past `covered`, which J56 raises as a finding
   * against a warehouse that did nothing wrong — and a claim on a line planning
   * already covered is refused outright with `OverCovers`. Both were reachable,
   * which is why `PickLine` now carries `picked` and `covered` and why the
   * shortfall is computed rather than assumed. `claim: 0` means the line is
   * already spoken for and the allocation call is skipped entirely.
   *
   * **Exactly one destination arm**, unchecked here because
   * `picking::landing` checks it once on the way in and the type below cannot
   * express both.
   *
   * The allocation's `id` comes off the same act as the event, so a second
   * press behind a lost response replays rather than minting a second claim —
   * Q174, and the reason `pickInto` mints its allocation id the same way.
   */
  async pick(input: {
    line: Uuid;
    stock: Uuid;
    to: { kind: "package" | "location"; id: Uuid };
    quantity: number;
    claim: number;
    act: Act;
  }): Promise<RecordPickResponse> {
    if (input.claim > 0) {
      await send<unknown>("POST", "/allocations", {
        id: input.act.id("allocation"),
        fulfilment_line_id: input.line,
        stock_id: input.stock,
        quantity: input.claim,
      });
    }
    return send<RecordPickResponse>("POST", "/picks", {
      from_stock_id: input.stock,
      fulfilment_line_id: input.line,
      ...(input.to.kind === "package"
        ? { to_package_id: input.to.id }
        : { to_location_id: input.to.id }),
      quantity: input.quantity,
      client_event_id: input.act.id("event"),
      occurred_at: input.act.at,
    });
  },

  /**
   * Offer a photograph in support of a record. D140.
   *
   * **Two calls, and the first one is the act.** `POST /evidence` mints the
   * look and the link together and answers with the event the picture then
   * hangs off — the same shape a capture session has, and for the same reason:
   * a photograph with no event is a photograph nobody can say who took, when,
   * or of what.
   *
   * The face is `detail`, never one of the seven sides. D141 resolves the
   * picker's recognition picture from `front`, so evidence filed there would
   * become what the next person sent to that bay is shown as what to look for.
   */
  async attachEvidence(input: {
    record: { discrepancy_id: Uuid } | { goods_receipt_line_id: Uuid };
    subject: { package_id: Uuid } | { location_id: Uuid } | { item_id: Uuid };
    image: Blob;
    note?: string;
    act: Act;
  }): Promise<void> {
    const look = await send<RecordEvidenceResponse>("POST", "/evidence", {
      ...input.record,
      ...input.subject,
      // An item is the only subject that takes a level, and an evidence
      // photograph of one is a photograph of a single unit.
      ...("item_id" in input.subject ? { packaging_level: "each" } : {}),
      ...(input.note ? { note: input.note } : {}),
      client_event_id: input.act.id("event"),
      occurred_at: input.act.at,
    });
    await api.photograph(look.observation_event_id, "detail", input.image);
  },

  recordCapture: (input: {
    item?: Uuid;
    style?: Uuid;
    /** D139. A part takes no packaging level: there is one handle. */
    part?: Uuid;
    level: string | null;
    /** D138. What state the thing was in. Required by the writer for a length
     *  at `each`, where arranging the thing is part of measuring it. */
    presentation?: string;
    measurements: {
      metric: string;
      entered_value?: string;
      unit?: string;
      absent_reason?: string;
    }[];
    act: Act;
  }) =>
    send<RecordObservationResponse>("POST", "/observations", {
      ...(input.item ? { item_id: input.item } : {}),
      ...(input.style ? { item_style_id: input.style } : {}),
      ...(input.part ? { item_part_id: input.part } : {}),
      ...(input.presentation ? { presentation: input.presentation } : {}),
      packaging_level: input.level,
      measurements: input.measurements,
      method: "instrument",
      ingestion_channel: "keyed",
      client_event_id: input.act.id("event"),
      occurred_at: input.act.at,
    }),

  /**
   * A photograph of one face, against the event the figures produced.
   *
   * **Raw bytes, not a JSON envelope.** Base64 inflates every upload by a
   * third over a warehouse's wifi to save the client a `fetch` option, and the
   * server reads the type from the bytes rather than believing this one — a
   * file stored as one thing and served as another is how an image endpoint
   * becomes an XSS, so the content type here is a hint and not a claim.
   */
  async photograph(event: Uuid, face: string, image: Blob): Promise<void> {
    const response = await transport.fetch(
      `${transport.base}/observations/${event}/images/${face}`,
      {
        method: "POST",
        credentials: transport.credentials,
        headers: {
          ...transport.headers(),
          "content-type": image.type || "application/octet-stream",
        },
        body: image,
      },
    );
    if (!response.ok) {
      let detail = `That photograph did not go up (${response.status}).`;
      try {
        const parsed = (await response.json()) as { detail?: string; error?: string };
        detail = parsed.detail ?? parsed.error ?? detail;
      } catch {
        /* an empty body is its own answer */
      }
      throw new ApiError(detail, response.status);
    }
  },

  seal: (carton: Uuid, act: Act) =>
    send<unknown>("POST", `/packages/${carton}/seal`, {
      client_event_id: act.id("event"),
      occurred_at: act.at,
    }),

  discard: (carton: Uuid, act: Act) =>
    send<unknown>("POST", `/packages/${carton}/void`, {
      client_event_id: act.id("event"),
      occurred_at: act.at,
    }),
};
