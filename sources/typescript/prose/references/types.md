# Types

What a `type` claim carries, and how a shape is read from TypeScript. `signature` is the declaration's source spelling; the claim's `path` anchors the declaration. Emit one per interface, type alias, class, enum, or DTO the surface takes, returns, persists, or publishes.

## Copy, do not describe

Carry the declaration as written — `interface User { id: string; email: string; createdAt: Date }` — never a paraphrase of it. A type written from memory drifts: `number` for a `string` id, an optional field made required, a union flattened to its first member.

## Follow the shape all the way down

- **Nesting.** A field typed by another interface is that interface's shape; claim it too, so the whole shape is in the Evidence.
- **Arrays and records.** `Array<{ id: string; tags: string[] }>`, `Record<string, Entry>` — carry the element type.
- **Unions and discriminants.** `{ success: true; data: Data } | { success: false; error: string }` — every variant, and the field that selects between them.
- **Enums and literal unions.** The member names and the values they serialise to.
- **Optionality.** `field?:`, `| undefined`, `| null` in the declaration; and in the code, `obj?.field`, `field ?? default`, `a || b`, `if (field != null)` — a field the handler guards is optional at runtime whatever the declaration says, and a fallback (`trainUpdate.evenTrainId || trainUpdate.oddTrainId`) makes both fields optional. Say so in the synopsis when the code and the declaration disagree.
- **Imported types.** A type from another module or package (`new SmarTrakEvent()`, a shared DTO) is claimed from its declaration where the tree holds it, with the surface's use in the synopsis: which fields this surface populates and which it leaves. A declaration the tree does not hold is not invented.

## Wire names and converters

Where a decorator maps a property to a wire name — `class-transformer` `@Expose({ name: "haEntrado" })`, `ta-json` `@JsonProperty("horaEntrada")`, a `@Transform` or `@JsonConverter` — the bare signature loses the mapping. Carry the decorated declaration verbatim, decorators included, and where a converter transforms the value (`"true"` / `"false"` to boolean) an `excerpt` of the converter, with the type's synopsis naming it. Note the format the type crosses the wire in (JSON, an XML root element) when the source says. `@Exclude()`, and a one-way `@Expose({ toClassOnly: true })` or `@Transform(…, { toPlainOnly: true })`, change the shape in one direction only — a field stripped from the outbound response, or accepted on input and never emitted — so the synopsis says which direction.

## What a response actually is

The runtime shape of a response is what the code deserialises and reads, not what an interface declares. `const allocated: string[] = await response.json()` makes the response `string[]`; a body read as `body.data.items` is that shape. Claim the shape the code uses, and say so when the declared type is broader. A cast is not a check: `response.data as User` and `process.env.KEY!` assert a shape or a presence without verifying it, where a `zod` or `class-validator` parse verifies it and rejects what fails — the synopsis says which the code does.
