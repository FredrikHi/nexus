# .NET client

Reports the outcome and duration of outbound calls to Nexus. It cannot break or
slow the application it watches.

Targets `net8.0` and `net10.0`. Two dependencies, both Microsoft abstractions.

## Get started

Create an ingest key under **Administration, API keys** with *Create unknown
integrations* ticked, then:

```bash
NEXUS_URL=https://nexus.example.com
NEXUS_API_KEY=iop_...
```

```csharp
builder.Services.AddNexus();
```

`AddNexus` reads those two variables, so a deployment configures it without a
code change. Leave the key unset and the client is inert: no queue, no timer,
no requests, which is what you want in tests and local runs.

## The easy win: report every outbound call

If you use `IHttpClientFactory`, one line per client reports everything it does,
and no call site has to know Nexus exists:

```csharp
builder.Services.AddHttpClient("stripe", c => c.BaseAddress = new Uri("https://api.stripe.com"))
                .AddNexusTelemetry("orders-to-stripe");
```

Every request through that client is now recorded with its status, duration and
operation, including requests added later by someone who has never heard of
this library. A 500 does not throw, so the handler reads the status code rather
than waiting for an exception: wrapping only the call would record every failed
request as a success.

The operation name is the method and path with the query string removed, since
a query carries ids and would make every request its own operation.

One client serving several integrations can say so per request:

```csharp
request.Options.Set(NexusTelemetryHandler.IntegrationOption, "refunds-to-stripe");
```

## Wrapping anything else

For SDK calls, database work, or anything that is not an `HttpClient`:

```csharp
var charge = await nexus.TrackAsync(
    "orders-to-stripe",
    () => stripe.Charges.CreateAsync(options),
    operation: "POST /charges");
```

`TrackAsync` returns whatever the call returned and rethrows exceptions
unchanged, so wrapping never alters the behaviour of the code around it.

## Traces come free

ASP.NET Core already opens an `Activity` per request, so the client reads
`Activity.Current` and every call made while handling one request shares a trace
id. Nothing to plumb through, no ambient context to set up.

Outside a web request, start one yourself:

```csharp
using var activity = new Activity("nightly-sync").Start();
```

## How outcomes are decided

| What happened | Recorded as |
| --- | --- |
| Returned | `SUCCESS` |
| `TimeoutException`, a cancelled call, 408, 504 | `TIMEOUT` |
| Any other 4xx, including 429 | `REJECTED` |
| 5xx, or anything else thrown | `FAILURE` |

`REJECTED` is deliberately separate from `FAILURE`. A rate limit means the
dependency is working and refusing you, and counting that as an outage makes a
noisy client look like a broken service. Only `FAILURE` and `TIMEOUT` count
toward the error rate that health is judged on.

`HttpRequestException.StatusCode` is read automatically. For an SDK that wraps
HTTP without exposing the code, put it where the client will find it:

```csharp
exception.Data["StatusCode"] = 429;
```

## What it guarantees

- **It never breaks your app.** Every path swallows its own errors. A dead
  collector is invisible to the request that produced the event.
- **It never slows a request down.** Recording costs a channel write; sending
  happens on a timer.
- **It drops telemetry rather than degrading you.** The queue is bounded and
  drops its oldest entries, counted by `DroppedCount`.
- **It gives up on what cannot succeed.** A batch Nexus refuses with a 4xx is
  dropped rather than retried forever ahead of everything behind it.

## Configuration

| Option | Default | |
| --- | --- | --- |
| `Url` | `NEXUS_URL` | Your instance |
| `ApiKey` | `NEXUS_API_KEY` | Ingest key. It can write telemetry and read nothing. |
| `Enabled` | true when a key is present | Set false to make everything a no-op |
| `FlushInterval` | 10s | Lower means fresher health and more requests |
| `MaxBatch` | 500 | The server refuses batches over 1000 |
| `MaxQueue` | 10000 | Past this the oldest events are dropped |
| `SendTimeout` | 10s | A slow collector must not become a slow shutdown |
| `OnError` | none | Called when a flush fails. Log it; do not throw. |

```csharp
builder.Services.AddNexus(options =>
{
    options.FlushInterval = TimeSpan.FromSeconds(5);
    options.OnError = e => logger.LogWarning(e, "telemetry flush failed");
});
```

## Shutting down

`NexusClient` is `IAsyncDisposable` and drains what is queued on disposal, so a
host that disposes its services loses nothing on a clean stop.
