# Documentation survey

This prompt runs once per bound `documentation` source, before anything is extracted. The caller has read every document already: the message lists each one with its outline — its length, every heading at its line, whether its body holds lists, tables, steps, quotations, or code — and lays the documents into the message whole, every line numbered, as far as they fit. Your job: from the outlines and the documents, name each subject the tree documents, anchor it at the heading, or the first line, that introduces it, and give it the stem its requirements lead with; list each document that introduces no subject as `unreached`. You group nothing and extract nothing. The caller derives the rest from the anchors you name — the lines each subject spans, the id its claims lead with, the seams the [extract prompt](extract.md) mines, whose answers the engine reconciles with every other source's into the specification — see [From sources to a spec](reconciliation.md).

The message is written in the caller's words for a source of code, and they map onto a documentation tree one to one: a *module* is a document; a *surface* is a subject; where a surface is *registered or declared* is where a subject is introduced — its heading, or its first line; *dead code* is a document that introduces no subject. Read every such word that way.

Everything this call needs is in this prompt and the message. The references `read_doc` offers are written for the extract call; load one only where a rule here links it — [claims.md](claims.md) for the `path` grammar — and never to decide what a subject is.

## Inputs

- **The documents** — every document of the tree, in path order, each with its outline: its line count; each heading at its line with its level (`L11 ## Membership`); and whether it holds lists, tables, steps, quotations, or code blocks, or prose alone. A document with no heading shows the line it opens at and what that line says. An `anchor` names one of these documents and nothing else.
- **The laid documents** — the leading documents laid out whole, every line led by its number: cite `#L<n>` from the numbers shown rather than reading them again. A document listed but not laid is read from `$SOURCE_DIR` only where its outline leaves its subject unclear.
- **`$SOURCE_DIR`** — read-only view of the bound tree, the whole of it, for a document the message lists rather than lays out.

Nothing outside the bound source is reachable; writes back into `$SOURCE_DIR` are denied.

## What a subject is

A subject is one thing the documented system does or governs for the people who use it — what a reviewer would put a heading over and write requirements under: a feature (resetting a password), a resource (orders, accounts), a flow (returning a tool), a policy (late returns), a recorded decision (append-only entries), a part of the design (the module layout). It is the grain the domain's own nouns give, not the grain of the writing:

- A document that treats one subject throughout — a guide to returning a tool, a policy on late returns, a decision record, a story file about invoices — is one subject, anchored at the document's title, or at its first line where it has no heading.
- A document that treats several — a reference page whose sections are membership, signing in, and resetting a password — is one subject per section, each anchored at the heading of its section; what comes before the first of them belongs to the first.
- A subject several documents treat — a guide, a reference page, and a policy on returns — is named in each document, at the heading that introduces it there, every one under the same stem; the caller mines them together.
- A guide's subject is what it walks through, not the walking: *Getting started* is about joining, so its subject is membership; *Returning a tool* is returns.
- Several stories, scenarios, or records under one heading are one subject where they are about one thing (three stories about invoices) and several where the headings keep them apart.

What introduces no subject is `unreached`: an index or readme that points at other documents, a glossary, a changelog, a table of error codes, a licence, a contributing guide, and every document about working on the repository or the documentation rather than about the system — installing, running, building, testing, releasing, the project's own command-line tooling. A document that explains how the system is put together — its architecture, its data model, its persistence — is a subject: its decisions are claims.

## Anchors

A subject's `anchor` is where the document introduces it, in the claim `path` grammar of [claims.md](claims.md): the document, relative to `$SOURCE_DIR`, and the line of its heading — `reference/accounts.md#L43` — or, where the document has no heading, the line it opens at — `the-return.md#L1`. One line, the heading's or the first; the caller reads the span from there to the next subject's anchor in the same document, or to the end. Cite the lines the outline gives. A document alone, with no line, anchors a subject the whole document introduces at its first line. No two subjects anchor at one line.

## Stems

The stem is the first segment of every `requirement` and `criterion` id the subject's extraction answers, and the engine slices the build plan by it, so two runs over one tree must name the same stems, and two documents about one subject the same one. Derive each by this convention, lowercase kebab-case, and by nothing else:

- the stem is the **domain noun** for the subject as the documents spell it — the word they use for the thing when they are not heading a section: `orders`, `membership`, `returns`, `password-reset`, `invoices`; plural where the documents say the plural, singular where they say the singular;
- a heading is a phrase about the noun, and the noun is what is kept: *Resetting a forgotten password* → `password-reset`; *Returning a tool* and *Late returns policy* and *Returns* → `returns`; *Joining Toolshed*, *Membership*, and *Membership eligibility policy* → `membership`; *Signing in* → `sign-in`;
- a document that names the thing by the system's own identifier, and by nothing else — a page headed `OrderService` whose sections are its operations — spells its noun that way, and the stem is that name kebab-cased: `order-service`;
- a decision record's stem is the thing the decision governs — *Append-only entries* → `entries`; *Idempotent posting* → `posting`; *Amounts in minor units* → `amounts` — not the record's number and not `decisions`;
- never a file name, a directory name, or a document kind: not `guides`, `reference`, `policies`, `adr`, `stories`, `docs`, `readme`.

Subjects on one noun share one stem — the guide, the reference page, and the policy on returns are three subjects under `returns` — and the caller tells them apart by the headings you anchored them at.

## Unreached documents

Every document is a subject's entry or it is not. List under `unreached` each document that introduces no subject by the rules above. The caller checks the coverage: a document in neither set comes back as a finding — name the subject it introduces, anchored at its heading or first line, or list it. Never invent a subject to cover a document.

## Output

One JSON object matching the survey schema: `surfaces`, each with a `name`, an `anchor`, and a `stem`; and `unreached`, the documents that introduce no subject, empty when every document does.

Rules:

- Every `anchor` names a document the message lists, relative to `$SOURCE_DIR`, at a line it holds — its heading's, or its first.
- Every `stem` is lowercase kebab-case, derived by the convention above.
- Every subject has a name, and no two subjects share one; the name is the subject as a reviewer would head it — `Returning a tool`, `Membership`, `Password reset` — never the document's path.
- No two subjects anchor at one line.
- An empty `surfaces` is the answer when no document introduces a subject — a tree of indexes, changelogs, and build notes. Do not invent one; the caller then mines the tree by directory.

## Worked example

A tool library's documentation, bound as the source `docs`. The message lists:

> - `README.md` — 14 lines; lists
>   - L1 # Toolshed documentation
> - `guides/returning-a-tool.md` — 40 lines; lists
>   - L1 # Returning a tool
>   - L9 ## At the counter
>   - L24 ## Returning late
> - `policies/late-returns.md` — 52 lines; tables
>   - L1 # Late returns policy
>   - L10 ## The grace period
>   - L21 ## Fees
>   - L38 ## Suspension
> - `reference/accounts.md` — 78 lines; lists, tables
>   - L1 # Accounts
>   - L11 ## Membership
>   - L43 ## Signing in
>   - L62 ## Resetting a forgotten password
> - `reference/glossary.md` — 61 lines; prose alone
>   - L1 # Glossary

and lays the five documents whole. `README.md` points at the other four and states nothing of its own; the glossary defines the words the rest use.

Resulting survey answer:

```json
{
  "surfaces": [
    { "name": "Returning a tool", "anchor": "guides/returning-a-tool.md#L1", "stem": "returns" },
    { "name": "Late returns policy", "anchor": "policies/late-returns.md#L1", "stem": "returns" },
    { "name": "Membership", "anchor": "reference/accounts.md#L11", "stem": "membership" },
    { "name": "Signing in", "anchor": "reference/accounts.md#L43", "stem": "sign-in" },
    { "name": "Password reset", "anchor": "reference/accounts.md#L62", "stem": "password-reset" }
  ],
  "unreached": ["README.md", "reference/glossary.md"]
}
```

Five subjects. The guide and the policy each treat one subject throughout, so each is anchored at its title, and both are returns — the one noun, however each document heads it — so both carry `returns`; their `##` sections are the subject's parts, not subjects of their own. `reference/accounts.md` treats three things in three sections, so it is three subjects at three headings, and what precedes `## Membership` — the page's introduction — belongs to the first; `Resetting a forgotten password` is a phrase about the noun, and the noun is kept. The readme and the glossary introduce no subject and are listed, not named.

## Anti-patterns

- **Naming a document.** `reference/accounts.md` is not a subject; membership, signing in, and password reset are. A document is a subject's entry, not the subject.
- **Naming every heading.** `## At the counter` and `## Returning late` are parts of returning a tool; a subject is the thing, not each section about it.
- **Cutting by directory or kind.** `guides/*` is not a subject, nor `policies/*`, nor "the reference"; a subject is what the system does, wherever it is written up.
- **A stem from a path.** `guides`, `reference`, `adr`, `stories` are where the documents sit, not what they are about.
- **A stem per heading.** *Returning a tool* and *Late returns policy* are both `returns`; a different stem for each breaks the one subject across two build slices.
- **Anchoring inside the body.** The anchor is the heading or the first line, where the subject is introduced, not a line that states a rule about it; the rules are the extract call's.
- **Inventing a subject.** A readme, an index, a glossary, a changelog, a build guide is listed under `unreached`; a tree of nothing else is answered `surfaces: []`.
- **Extracting.** This call names subjects and nothing else; claims are the extract call's.

## Failure modes

| Condition | Action |
| --------- | ------ |
| The tree is one document | Answer its subjects — one at its title where it treats one thing, one per section where it treats several. |
| A document has no heading | Anchor its subject at the line it opens at, and read what it is about from the laid text. |
| One subject runs through several documents | Name it in each, at the heading introducing it there, under the one stem. |
| A document's sections are about several things | One subject per section at its heading; the introduction before the first belongs to the first. |
| Several stories or scenarios in one document | One subject where they are about one thing; one per heading where the headings keep them apart. |
| The documents are decision records | One subject per record, at its title, under the noun of what it governs; their index is unreached. |
| A document is about the repository, the build, or the documentation itself | Unreached, however long. |
| No document introduces a subject | Answer `surfaces: []` and list nothing as unreached; the caller mines the tree by directory. |
| A document is neither a subject's entry nor listed unreached | The caller returns it as a finding: anchor the subject it introduces, or list it. |
| The answer names a document the tree lacks, a stem outside kebab-case, a subject twice, or two subjects at one line | The caller rejects it and asks again with the findings; correct the named subjects. |
