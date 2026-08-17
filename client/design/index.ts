/**
 * The design package's public surface.
 *
 * Everything a screen may use is here, and nothing else is exported. The
 * material stylesheets are deliberately absent: there is no supported way
 * to import `anodise.module.css` and compose part of it, because there is
 * no part of it that is still anodised aluminium (D123).
 */

export { LightRoom, useRelight } from "./light/LightRoom";
export { startLightSolver, type LightSolver } from "./light/solver";

export { Panel, Well } from "./primitives/Panel";
export { Nylon, Boot, Tag } from "./primitives/Nylon";
export { Key } from "./primitives/Key";
export { Link } from "./primitives/Link";
export { Badge } from "./primitives/Badge";
export { Lamp, type LampKind } from "./primitives/Lamp";
export { Readout, Dim } from "./primitives/Readout";
export {
  Face,
  FaceWell,
  Band,
  Code,
  Soft,
  Faint,
  Pill,
  Finding,
  EvidencePair,
  Field,
  Table,
  Num,
  NumHead,
  Action,
  Chooser,
  Steel,
} from "./primitives/Face";

export { EmptySlot } from "./primitives/EmptySlot";
export { Photo, NoPhoto } from "./primitives/Photo";
export { ScanInput } from "./primitives/ScanInput";
export { Tabs } from "./primitives/Tabs";

export { Stack, Row, Grid, Spacer, Trailing, type Gap } from "./layout/Stack";

/* Patterns: the compositions the screens kept re-deriving, decided once.
   A material is what a thing is made of; a pattern is what it is. */
export { Record, Records, Fact } from "./patterns/Record";
export { Notice } from "./patterns/Notice";

export { groupDigits, grams, millimetres, centimetres } from "./format";
export type { Elevation } from "./cx";

export {
  TOKENS,
  THEMED_TOKENS,
  FIXED_TOKENS,
  SOLVER_TOKENS,
  token,
  type Token,
  type ThemedToken,
  type SolverToken,
} from "./tokens.gen";
