// The command line and the replacement rule every case generator shares.
//
// Generation writes into the checkout. A case that already has a manifest is
// replaced only when the caller passes --supersede, so a stray `generate`
// cannot silently rewrite reviewed evidence; reproduction never writes the
// checkout. A superseded manifest keeps the approval block it had: ADR-0019
// makes that block historical metadata, and carrying it keeps the Git diff of
// a regeneration to what the reference moved.
import { existsSync, readFileSync } from "node:fs";

/** `<generate|reproduce> <case-id> [--supersede]`, validated. */
export function generationArgs(argv, usage) {
  const [mode, caseId, ...flags] = argv;
  const supersede = flags.length === 1 && flags[0] === "--supersede";
  if (
    (mode !== "generate" && mode !== "reproduce")
    || !caseId
    || (flags.length > 0 && !supersede)
    || (supersede && mode !== "generate")
  ) {
    throw new Error(`usage: ${usage} <generate|reproduce> <case-id> [--supersede with generate]`);
  }
  return { mode, caseId, supersede };
}

/**
 * The approval block a generated manifest carries: the replaced manifest's
 * when superseding, `null` for a new case. Refuses to generate over a
 * manifested case without --supersede.
 */
export function carriedApproval(manifestPath, { mode, caseId, supersede }) {
  if (mode !== "generate" || !existsSync(manifestPath)) {
    return null;
  }
  if (!supersede) {
    throw new Error(
      `${caseId} is manifested; generation refuses to replace it without --supersede`,
    );
  }
  return JSON.parse(readFileSync(manifestPath, "utf8")).approval ?? null;
}
