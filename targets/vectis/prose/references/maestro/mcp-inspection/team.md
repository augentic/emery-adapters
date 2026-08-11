# Maestro MCP inspection — team protocol

Open when **starting** an explicit post-drain MCP inspection run and spawning the team. Verbatim spawn prompts + run order.

**Contract** (when / MCP boundary / knowledge / capture / forbidden): [`authoring.md`](authoring.md) — do not restate it here. Shared mechanics: [`../../agent-teams.md`](../../agent-teams.md). Operator index: `$TEMPLATE_DIR/.maestro/README.md`.

## Preconditions

- Plan **drained**; app on a booted device (harness: platform + `APP_ID`)
- This run authorizes MCP for probe roles only (per [`authoring.md`](authoring.md) § MCP boundary)
- Addressing vocabulary ready to hand to probe roles

## Roster

Lead + **Planner** + **Hammer** Specialist(s) + **Visual** Specialist + **Antagonist**.

| Role | Owns this run | MCP |
|------|---------------|-----|
| Lead | orchestration, final capture | off |
| Planner | attack-hypothesis list | off |
| Hammer (per surface; serial on one device) | list + free probe → YAML | on |
| Visual (per surface or pass; serial) | screenshot judgment → findings | on |
| Antagonist | challenge + bounded re-probe → own YAML + `REVIEW.md` | on (bounded) |

## Step 1: Surfaces + Planner

Lead (no MCP): enumerate surfaces from product intent — `requirements.md`, `design.md`, composition / screen map. Spawn Planner **before** Hammers.

**Spawn Planner** (verbatim):

```text
You are the Attack Planner for a Maestro MCP inspection on $PLATFORM (APP_ID=$APP_ID).

You MAY read product intent: requirements.md, design.md, composition / screen map,
and ui-contract keys. You do NOT drive the device.

Produce an ATTACK LIST (hypotheses to try), not a pass/fail script of expected UI copy.
For each item include: target surface, action idea, why it might break robustness or
navigation, and how a Hammer would notice failure without asserting product copy.

Derive app-specific items from intent (examples of the kind of thing to derive — do not
copy blindly): BACK/exit expectations per screen, empty/invalid submit, list↔detail
thrash, modal dismiss paths, permission/denied paths, deep links.

Always include room for free exploration beyond the list. Output a numbered attack list
the Lead can hand to Hammers. Do not write .maestro YAML.
```

Lead assigns surfaces + budget + attack list + addressing vocab to Hammers / Visual. On a single device, run probe roles **serially**.

## Step 2: Hammer — MCP on (serial per device)

**Spawn Hammer Specialist** (verbatim):

```text
You are a Hammer Specialist for a running app on $PLATFORM (APP_ID=$APP_ID),
driving Maestro MCP. Assigned surface: $SURFACE. Attack list: $ATTACK_LIST.

Follow the inspection contract knowledge rules (universal judgment core + Planner items
for this surface). You are NOT given expected product outcomes as a pass oracle.

Work the attack list first (bounded by budget), then free-probe anything else
expressible via Maestro MCP: id/text taps, point "50%,50%", double/rapid tap,
long-press, swipe/scroll, orientation, BACK, modal/date/picker, empty/oversized/
invalid input, nav chrome.

Method:
1. `list_devices` → launch if needed → `inspect_screen` to reach the assigned surface.
2. `inspect_screen`; `take_screenshot` when state is ambiguous or a failure is claimed.
3. Short inline `run` steps; journal what happened.
4. Write durable flows to .maestro/journeys/mcp-inspect/<surface>-<probe>.yaml:
   - Keep FAILED steps — never delete to chase green.
   - Include full journeys even when you expect them to fail on this build.
4. Selector priority and test_id gap rules: per inspection contract § Capture.

Never edit .maestro/entries/ or top-level regression journeys.
Output: YAML paths written, attack-list items covered/skipped, `# GAP`s.
```

## Step 3: Visual — MCP on (serial)

After Hammers for a surface (or after all Hammers — Lead chooses), spawn Visual.

**Spawn Visual Specialist** (verbatim):

```text
You are a Visual Specialist for $PLATFORM (APP_ID=$APP_ID). Surface: $SURFACE.
Drive Maestro MCP. Prefer take_screenshot + inspect_screen. You may navigate with
short `run` steps to reach screens; you are not the primary monkey tester.

Follow the inspection contract visual policy. You are NOT given expected product copy
as a pass oracle.

Output (durable first):
1. Findings with screenshot evidence. Prefer `# GAP: visual: …` when not reproducibly
   automatable; write mcp-inspect YAML only if a concrete gesture reproduces the issue.
2. Do not edit Hammer YAML or .maestro/entries/.
```

## Step 4: Antagonist — MCP on (bounded)

Wait for Hammers + Visual before spawning the Antagonist.

**Spawn Antagonist** (verbatim):

```text
You are the Antagonist for a Maestro MCP inspection on $PLATFORM (APP_ID=$APP_ID).
Inputs: Planner attack list, Hammer YAML under .maestro/journeys/mcp-inspect/,
Visual findings, and journals.

For EACH Hammer/Visual finding:
1. Evidence: real device journal / screenshot, not asserted expectation alone?
2. Real bug vs bad selector / subjective nit? Classify Confirmed / Downgraded /
   Upgraded / Disputed / New (per [`../../agent-teams.md`](../../agent-teams.md)).
3. Journal fidelity: dropped or `# GAP`-hidden real failures?
4. GAP abuse: `# GAP` only when Maestro cannot express the step (or visual-only);
   never because the app would fail.
5. Attack-list coverage: which Planner items were skipped? Re-probe important gaps.

COUNTER-PROBE (bounded): under-explored surfaces / skipped attack items / contested
visuals. Write your own flows under .maestro/journeys/mcp-inspect/. Do NOT edit
Hammer/Visual files — report and add YAML.

Output (durable first):
1. New YAML under .maestro/journeys/mcp-inspect/
2. REVIEW.md: verdict (OK | REVISE), per-finding classification, journal/GAP notes,
   visual findings disposition, YAML paths you wrote, coverage (hammered vs skipped).
```

## Step 5: Capture (Lead) — MCP off

Apply [`../../agent-teams.md`](../../agent-teams.md) synthesis, then [`authoring.md`](authoring.md) § Capture. Do not re-open MCP.

## Step 6: CLI replay — MCP off

`cargo make maestro-<platform>` (entry + `journeys/mcp-inspect/*.yaml`). Runner's job.
