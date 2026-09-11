# Node client

Zero dependencies, one file. Copy `src/observability.ts` into your project, or
build this package and import it. It needs `fetch` and `AsyncLocalStorage`,
both of which Node has had since 18.

## What it guarantees

- **It never breaks your app.** Every path swallows its own errors. A dead
  collector is invisible to the request that produced the event.
- **It never slows a request down.** Recording costs an array push; sending
  happens on a timer.
- **It drops telemetry rather than degrading you.** The queue is bounded. When
  it fills, the oldest events go and `droppedCount()` counts them.

## Getting the integration ids

Model the landscape from a file, and let the script hand you the ids:

```bash
node scripts/apply-landscape.mjs examples/cookly.landscape.json \
  --token "$ICC_TOKEN" --org "$ICC_ORG_ID" \
  --emit src/lib/integrations.ts
```

Re-running only creates what is missing, so keep the JSON in version control
next to the code it describes and re-apply when the landscape changes.

The generated file exports ids rather than names, so renaming an integration in
the UI does not touch the code reporting against it.

## Wiring it into Fastify

`initObservability` returns the live instance. Importing `observability` is
fine at a call site, because the import is a live binding — but destructuring
it out of the module before init captures the inert placeholder, which queues
nothing and flushes nothing. In a script, keep what init hands back.

```ts
// src/plugins/observability.ts
import fp from 'fastify-plugin'
import { initObservability, observability } from '../lib/observability'

export default fp(async (fastify) => {
  initObservability({
    url: process.env.ICC_URL!,
    apiKey: process.env.ICC_API_KEY!,
    // Off unless configured, so local runs and CI send nothing.
    enabled: Boolean(process.env.ICC_API_KEY),
    onError: (error) => fastify.log.warn({ error }, 'telemetry flush failed'),
  })

  // One trace per request. Everything tracked beneath it correlates without
  // any call site having to know the trace exists.
  fastify.addHook('onRequest', (request, _reply, done) => {
    observability.withTrace(request.id, done)
  })

  fastify.addHook('onClose', async () => {
    await observability.close()
  })
})
```

## Wrapping the calls that matter

`track` returns whatever the call returned and rethrows errors unchanged, so
wrapping never alters behaviour.

```ts
// src/services/recipe-extraction-service.ts
import { observability } from '../lib/observability'
import { INTEGRATIONS } from '../lib/integrations'

const completion = await observability.track(
  INTEGRATIONS.RECIPE_EXTRACTION_OPENAI,
  () => openai.chat.completions.create({ model: 'gpt-4o-mini', messages }),
  { operation: 'POST /chat/completions' },
)
```

Prisma is one wrapper for every query, using a client extension:

```ts
// src/lib/prisma.ts
export const prisma = new PrismaClient().$extends({
  query: {
    $allModels: {
      async $allOperations({ model, operation, args, query }) {
        return observability.track(
          INTEGRATIONS.PRISMA_POSTGRES,
          () => query(args),
          { operation: `${model}.${operation}` },
        )
      },
    },
  },
})
```

Do the same at the other boundaries: `src/lib/object-storage.ts` for S3,
`sendEmail` in `src/lib/utils.ts` for Resend, `src/lib/push-notification.ts`,
`src/lib/revenuecat.ts` and `src/lib/redis.ts`.

## Two things worth knowing before you wrap

**A client that resolves on failure needs the check inside the wrapper.**
Resend returns `{ error }` rather than throwing, Expo answers 200 with an error
ticket, and `fetch` resolves happily on a 500. Wrapping only the call records
every one of those as a success. Throw inside the tracked function and catch it
immediately after, and the recorded outcome is right while the surrounding
control flow is untouched. Put `status` on the error you throw: it is what
separates a refusal from an outage.

**Some SDKs already report.** Stripe emits a `response` event carrying the
method, path, status and elapsed time for every request it completes, so one
listener covers every call site and nothing can be forgotten later:

```ts
stripe.on('response', (event) => {
  observability.record({
    integration_id: INTEGRATIONS.PAYMENTS_STRIPE,
    status: event.status >= 500 ? 'FAILURE' : event.status >= 400 ? 'REJECTED' : 'SUCCESS',
    status_code: event.status,
    duration_ms: event.elapsed,
    operation: `${event.method} ${event.path}`,
  })
})
```

Its blind spot is a request that never got a response: a dropped connection
raises an error without emitting anything, so those show up as a gap rather
than a failure.

## Model a fallback as two integrations, not one

Where a primary provider fails over to a second one, report each leg against
its own integration. Cookly's text prompts go to Groq and fall back to OpenAI
on a 429, which is the normal working state rather than an incident. One shared
number would average the two into an outage on nothing in particular; two show
Groq shedding load and OpenAI absorbing it, which is what you actually want to
see.

## How outcomes are decided

| What happened | Recorded as |
| --- | --- |
| Resolved | `SUCCESS` |
| Aborted, `ETIMEDOUT`, 408, 504 | `TIMEOUT` |
| Any other 4xx, including 429 | `REJECTED` |
| 5xx, or anything else thrown | `FAILURE` |

`REJECTED` is deliberately separate from `FAILURE`. A rate limit or a validation
error means the dependency is working and refusing you, and counting that as an
outage would make a noisy client look like a broken dependency. Only `FAILURE`
and `TIMEOUT` count toward the error rate health is judged on.

## Configuration

| Variable | Meaning |
| --- | --- |
| `ICC_URL` | Base URL of the API |
| `ICC_API_KEY` | Ingest key from Administration. It cannot read anything. |

Leave `ICC_API_KEY` unset and the client is inert.
