# Example: parallel execution

## Scenario

The survey named one surface: the exported API `EventProcessor`, entered at `services/event-processor/index.ts`. The tree also holds `services/event-processor/publisher.ts`; the example shows the entry file alone, with the private method bodies elided.

## Source

`services/event-processor/index.ts`:

```typescript
import { getAzureToken } from "@azure/identity";
import { Publish } from "./publisher";

interface EventInput {
  id: string;
  type: string;
  data: object;
}

interface EnrichedEvent {
  id: string;
  type: string;
  data: object;
  enrichedData: object;
  metadata: object;
}

const config = { tenantId: process.env.AZURE_TENANT_ID! };

export class EventProcessor {
  constructor(private publisher: Publish) {}

  async process(input: EventInput): Promise<void> {
    try {
      const token = await getAzureToken(config.tenantId);
      const [enriched, metadata] = await Promise.all([
        this.enrichData(input, token),
        this.fetchMetadata(input.id),
      ]);
      await this.publisher.send("events-topic", enriched);
      await this.delay(5000);
      await this.publisher.send("audit-topic", { timestamp: Date.now(), metadata });
    } catch (error) {
      if (error.code === "AUTH_FAILED") {
        throw new Error("Authentication failed");
      }
      console.error("Processing error:", error);
    }
  }

  private async enrichData(input: EventInput, token: string): Promise<EnrichedEvent> {
    // …
  }

  private async fetchMetadata(id: string): Promise<object> {
    // …
  }

  private delay(ms: number): Promise<void> {
    return new Promise((resolve) => setTimeout(resolve, ms));
  }
}
```

## Evidence

```json
{
  "claims": [
    {
      "kind": "requirement",
      "id": "event-processing.authentication",
      "path": "services/event-processor/index.ts#L25",
      "statement": "Processing an event first acquires an Azure AD token for the tenant named by AZURE_TENANT_ID."
    },
    {
      "kind": "requirement",
      "id": "event-processing.tenant-config",
      "path": "services/event-processor/index.ts#L18",
      "statement": "The tenant id is read from AZURE_TENANT_ID once when the module loads and is asserted present rather than checked."
    },
    {
      "kind": "requirement",
      "id": "event-processing.enrichment",
      "path": "services/event-processor/index.ts#L26-L29",
      "statement": "Enrichment with the token and the metadata lookup by event id run in parallel, and publishing waits for both to complete."
    },
    {
      "kind": "requirement",
      "id": "event-processing.publication",
      "path": "services/event-processor/index.ts#L30-L32",
      "statement": "The enriched event is published once to events-topic; 5000 milliseconds later an audit record carrying the current timestamp and the metadata is published once to audit-topic."
    },
    {
      "kind": "requirement",
      "id": "event-processing.auth-failure",
      "path": "services/event-processor/index.ts#L34-L36",
      "statement": "An error whose code is AUTH_FAILED, from any step, is rethrown as Error(\"Authentication failed\") and nothing further is published."
    },
    {
      "kind": "requirement",
      "id": "event-processing.other-failure",
      "path": "services/event-processor/index.ts#L37",
      "statement": "Any other error is logged as \"Processing error:\" with the error and processing returns normally, so an event may reach events-topic with no audit record following it."
    },
    {
      "kind": "excerpt",
      "path": "services/event-processor/index.ts#L24-L38",
      "excerpt": "One try around the whole flow: token, Promise.all over enrichData and fetchMetadata, publish to events-topic, delay(5000), publish { timestamp: Date.now(), metadata } to audit-topic. The catch rethrows AUTH_FAILED alone and swallows everything else with console.error."
    },
    {
      "kind": "type",
      "path": "services/event-processor/index.ts#L4-L8",
      "signature": "interface EventInput { id: string; type: string; data: object }"
    },
    {
      "kind": "type",
      "path": "services/event-processor/index.ts#L10-L16",
      "signature": "interface EnrichedEvent { id: string; type: string; data: object; enrichedData: object; metadata: object }"
    },
    {
      "kind": "call",
      "path": "services/event-processor/index.ts#L25",
      "callee": "@azure/identity:getAzureToken",
      "synopsis": "acquire a token for config.tenantId"
    },
    {
      "kind": "call",
      "path": "services/event-processor/index.ts#L30",
      "callee": "services/event-processor/publisher.ts:Publish.send",
      "synopsis": "publish the enriched event to events-topic"
    },
    {
      "kind": "call",
      "path": "services/event-processor/index.ts#L32",
      "callee": "services/event-processor/publisher.ts:Publish.send",
      "synopsis": "publish { timestamp, metadata } to audit-topic after a 5000 ms delay"
    }
  ]
}
```

## What to notice

- Parallelism is stated where it is observable: nothing is published until both lookups complete. The order of the two publishes and the delay between them are behaviour, with the value the source spells.
- One `try` covers the whole flow, so the two arms of the `catch` are two requirements with different consequences, and the partial-publication consequence is stated because the code exhibits it.
- The audit payload is an inline literal with no declaration, so it has no `type` claim; its shape lives in the statement and the excerpt.
- The private method bodies are elided here. In a real tree the extract call follows them, and the outbound calls enrichment makes are this surface's `call` claims; nothing is claimed about what they do until it is read.
- `getAzureToken` comes from a package, so its `callee` is `<package>:<symbol>`; `Publish.send` is a method of the tree, so its `callee` is `<file>:<symbol>`.
