# Node client

Zero dependencies, one file. Copy `src/observability.ts` into your project, or
build this package and import it. It needs `fetch` and `AsyncLocalStorage`,
both of which Node has had since 18.

## Get started

Under **Administration, API keys**, create an ingest key with *Create unknown
integrations* ticked, then:

```bash
ICC_URL=https://your-instance
ICC_API_KEY=iop_...
```

```ts
import { initObservability, observability } from './lib/observability'

initObservability({
  url: process.env.ICC_URL!,
  apiKey: process.env.ICC_API_KEY!,
  // Off unless configured, so tests and local runs send nothing.
  enabled: Boolean(process.env.ICC_API_KEY),
})

const completion = await observability.track(
  'bruno-to-groq',
  () => groq.chat.completions.create(params),
  { operation: 'chat.completions.create' },
)
```

That is the whole setup. The first event naming `bruno-to-groq` creates that
integration, so there is nothing to model up front and nothing to generate.
Discovered integrations arrive under a placeholder system called **Unmapped**;
wire them to their real endpoints in the UI when you are ready. Renaming one
keeps its slug, so the code goes on working.

Turn auto-create off once the picture is complete. From then on a misspelled
slug is refused rather than quietly becoming a new integration.

## Naming integrations

A slug names one edge between two things: `orders-to-stripe`,
`prisma-to-postgres`, `bruno-to-groq`. Lowercase, dashes, stable.

Give each caller its own slug even when the dependency is shared. Cookly calls
OpenAI from six places, and six slugs mean a failure in recipe tagging does not
drag down the health of meal scanning. One shared slug would average them into
a number that describes nothing.

Where a call falls back to a second provider, report each leg separately.
`bruno-to-groq` and `bruno-to-openai` show Groq shedding load and OpenAI
absorbing it, which is the normal working state. Merged into one slug it would
look like an outage.

Keeping the slugs in a constants file rather than scattered literals is worth
it once you have more than a handful:

```ts
export const INTEGRATIONS = {
  BRUNO_GROQ: 'bruno-to-groq',
  BRUNO_OPENAI: 'bruno-to-openai',
} as const
```

## What it guarantees

- **It never breaks your app.** Every path swallows its own errors. A dead
  collector is invisible to the request that produced the event.
- **It never slows a request down.** Recording costs an array push; sending
  happens on a timer.
- **It drops telemetry rather than degrading you.** The queue is bounded. When
  it fills, the oldest events go and `droppedCount()` counts them.
- **It gives up on what cannot succeed.** A batch the collector refuses with a
  4xx is dropped rather than retried forever at the head of the queue.

## Wiring it into Fastify

`initObservability` returns the live instance. Importing `observability` at a
call site is fine, because the import is a live binding, but destructuring it
out of the module before init captures the inert placeholder, which queues
nothing and flushes nothing.

```ts
// src/plugins/observability.ts
import fp from 'fastify-plugin'
import { initObservability, observability } from '../lib/observability'

export default fp(async (fastify) => {
  initObservability({
    url: process.env.ICC_URL!,
    apiKey: process.env.ICC_API_KEY!,
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

Prisma is one wrapper for every query, using a client extension:

```ts
export const prisma = new PrismaClient().$extends({
  query: {
    $allModels: {
      async $allOperations({ model, operation, args, query }) {
        return observability.track('prisma-to-postgres', () => query(args), {
          operation: `${model}.${operation}`,
        })
      },
    },
  },
})
```

Do the same at the other boundaries: object storage, your mail sender, your
payment provider, your cache.

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
    integration: 'payments-to-stripe',
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

## Landscape as code

Auto-create is the fast way in. When you would rather describe the landscape
deliberately, and keep it reviewable in a pull request, apply a file instead:

```bash
node scripts/apply-landscape.mjs my-landscape.json \
  --url "$ICC_URL" --token "$TOKEN" --org "$ORG_ID"
```

Re-running creates only what is missing. `examples/cookly.landscape.json` is a
real one, covering nine systems and twenty-one integrations.

## Configuration

| Variable | Meaning |
| --- | --- |
| `ICC_URL` | Base URL of your instance |
| `ICC_API_KEY` | Ingest key from Administration. It cannot read anything. |

Leave `ICC_API_KEY` unset and the client is inert.
