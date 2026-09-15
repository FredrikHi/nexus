using System.Diagnostics;
using System.Net.Http.Json;
using System.Text.Json;
using System.Text.Json.Serialization;
using System.Threading.Channels;

namespace Nexus.Observability;

/// <summary>Raised inside the client when Nexus refuses a batch. Never reaches your code.</summary>
public sealed class NexusIngestException : Exception
{
    public NexusIngestException(int? statusCode, string message) : base(message) => StatusCode = statusCode;

    /// <summary>The HTTP status Nexus answered with, when there was a response.</summary>
    public int? StatusCode { get; }
}

/// <summary>
/// Reports outbound calls to Nexus.
///
/// Three rules shape everything here:
///
/// 1. It must never break the host application. Every path swallows its own
///    errors, and a dead collector is invisible to the request that produced
///    the event.
/// 2. It must never slow a request down. Events queue in memory and flush on a
///    timer, so recording one costs a channel write.
/// 3. Losing telemetry beats degrading the thing being observed. The queue is
///    bounded; when it fills, the oldest events are dropped and counted.
/// </summary>
public sealed class NexusClient : IAsyncDisposable
{
    private static readonly JsonSerializerOptions Json = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.SnakeCaseLower,
        DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull,
        Converters = { new JsonStringEnumConverter(JsonNamingPolicy.SnakeCaseUpper) },
    };

    private readonly NexusOptions _options;
    private readonly HttpClient _http;
    private readonly bool _ownsHttp;
    private readonly bool _enabled;
    private readonly Channel<TelemetryEvent>? _queue;
    private readonly CancellationTokenSource _stopping = new();
    private readonly Task _pump = Task.CompletedTask;
    private long _dropped;

    public NexusClient(NexusOptions options, HttpClient? httpClient = null)
    {
        _options = options ?? throw new ArgumentNullException(nameof(options));
        _enabled = options.IsEnabled;

        _ownsHttp = httpClient is null;
        _http = httpClient ?? new HttpClient();
        _http.Timeout = options.SendTimeout;

        if (!_enabled) return;

        // DropOldest is the bounded-queue rule, enforced by the channel rather
        // than by hand, and the callback is how a drop gets counted.
        _queue = Channel.CreateBounded<TelemetryEvent>(
            new BoundedChannelOptions(options.MaxQueue)
            {
                FullMode = BoundedChannelFullMode.DropOldest,
                SingleReader = true,
            },
            _ => Interlocked.Increment(ref _dropped));

        _pump = Task.Run(PumpAsync);
    }

    /// <summary>How many events have been dropped. Worth logging periodically.</summary>
    public long DroppedCount => Interlocked.Read(ref _dropped);

    /// <summary>Queues one event. Returns immediately and never throws.</summary>
    public void Record(TelemetryEvent telemetry)
    {
        if (!_enabled || _queue is null) return;

        telemetry.OccurredAt ??= DateTimeOffset.UtcNow;
        // ASP.NET Core already opens an Activity per request, so the trace id
        // is there to be read rather than plumbed through by hand.
        telemetry.TraceId ??= Activity.Current?.TraceId.ToString();

        if (!_queue.Writer.TryWrite(telemetry)) Interlocked.Increment(ref _dropped);
    }

    /// <summary>
    /// Times a call, records how it went, and returns whatever it returned.
    /// Errors are recorded and rethrown unchanged, so wrapping a call never
    /// alters the behaviour of the code around it.
    /// </summary>
    public async Task<T> TrackAsync<T>(
        string integration,
        Func<Task<T>> action,
        string? operation = null,
        Dictionary<string, object?>? metadata = null)
    {
        if (!_enabled) return await action().ConfigureAwait(false);

        var occurredAt = DateTimeOffset.UtcNow;
        var started = Stopwatch.GetTimestamp();

        try
        {
            var result = await action().ConfigureAwait(false);
            Record(new TelemetryEvent
            {
                Integration = integration,
                Status = TelemetryStatus.Success,
                OccurredAt = occurredAt,
                DurationMs = ElapsedMs(started),
                Operation = operation,
                Metadata = metadata,
            });
            return result;
        }
        catch (Exception exception)
        {
            var (status, statusCode, errorType, message) = Outcome.Classify(exception);
            Record(new TelemetryEvent
            {
                Integration = integration,
                Status = status,
                OccurredAt = occurredAt,
                DurationMs = ElapsedMs(started),
                Operation = operation,
                StatusCode = statusCode,
                ErrorType = errorType,
                ErrorMessage = message,
                Metadata = metadata,
            });
            throw;
        }
    }

    /// <summary>The same, for a call that returns nothing.</summary>
    public Task TrackAsync(
        string integration,
        Func<Task> action,
        string? operation = null,
        Dictionary<string, object?>? metadata = null) =>
        TrackAsync<object?>(integration, async () => { await action().ConfigureAwait(false); return null; },
            operation, metadata);

    private static int ElapsedMs(long started) =>
        (int)Stopwatch.GetElapsedTime(started).TotalMilliseconds;

    /// <summary>Sends whatever is queued. Safe to call at any time.</summary>
    public Task FlushAsync() => _enabled ? DrainAsync(CancellationToken.None) : Task.CompletedTask;

    private async Task PumpAsync()
    {
        using var timer = new PeriodicTimer(_options.FlushInterval);
        try
        {
            while (await timer.WaitForNextTickAsync(_stopping.Token).ConfigureAwait(false))
            {
                await DrainAsync(_stopping.Token).ConfigureAwait(false);
            }
        }
        catch (OperationCanceledException)
        {
            // Shutting down.
        }

        // A final drain, on its own budget rather than the cancelled token, so
        // the last window of events is not lost on a clean stop.
        await DrainAsync(CancellationToken.None).ConfigureAwait(false);
    }

    private async Task DrainAsync(CancellationToken cancellationToken)
    {
        if (_queue is null) return;

        while (true)
        {
            var batch = new List<TelemetryEvent>(Math.Min(_options.MaxBatch, 64));
            while (batch.Count < _options.MaxBatch && _queue.Reader.TryRead(out var item))
            {
                batch.Add(item);
            }

            if (batch.Count == 0) return;

            try
            {
                await SendAsync(batch, cancellationToken).ConfigureAwait(false);
            }
            catch (Exception exception)
            {
                if (IsPermanent(exception))
                {
                    // Nexus read the batch and refused it: a revoked key, or an
                    // integration that does not exist. Retrying sends the same
                    // bytes to the same answer forever, and would block every
                    // later event behind it.
                    Interlocked.Add(ref _dropped, batch.Count);
                    _options.OnError?.Invoke(exception);
                    continue;
                }

                // Retryable. Put them back and stop for this pass. Order is not
                // preserved, and does not need to be: every event carries its
                // own occurred_at and the server sorts on that.
                foreach (var item in batch)
                {
                    if (!_queue.Writer.TryWrite(item)) Interlocked.Increment(ref _dropped);
                }

                _options.OnError?.Invoke(exception);
                return;
            }
        }
    }

    private async Task SendAsync(IReadOnlyList<TelemetryEvent> events, CancellationToken cancellationToken)
    {
        using var request = new HttpRequestMessage(
            HttpMethod.Post, $"{_options.Url.TrimEnd('/')}/api/v1/telemetry")
        {
            Content = JsonContent.Create(new { events }, options: Json),
        };
        request.Headers.TryAddWithoutValidation("Authorization", $"Bearer {_options.ApiKey}");

        using var response = await _http.SendAsync(request, cancellationToken).ConfigureAwait(false);
        if (response.IsSuccessStatusCode) return;

        var body = await ReadBodyAsync(response).ConfigureAwait(false);
        throw new NexusIngestException(
            (int)response.StatusCode,
            $"telemetry ingest returned {(int)response.StatusCode}: {body}");
    }

    private static async Task<string> ReadBodyAsync(HttpResponseMessage response)
    {
        try
        {
            var body = await response.Content.ReadAsStringAsync().ConfigureAwait(false);
            return body.Length <= 200 ? body : body[..200];
        }
        catch
        {
            return "";
        }
    }

    /// <summary>
    /// Whether a failed send can never succeed on a retry. A 4xx means the
    /// batch was read and rejected, which waiting does not fix. 408 and 429 are
    /// the exceptions, being about timing rather than content.
    /// </summary>
    private static bool IsPermanent(Exception exception) =>
        exception is NexusIngestException { StatusCode: >= 400 and < 500 and not 408 and not 429 };

    /// <summary>Flushes and stops. Call this on shutdown.</summary>
    public async ValueTask DisposeAsync()
    {
        if (_enabled)
        {
            _queue?.Writer.TryComplete();
            await _stopping.CancelAsync().ConfigureAwait(false);
            try
            {
                await _pump.ConfigureAwait(false);
            }
            catch
            {
                // Shutdown must not throw.
            }
        }

        _stopping.Dispose();
        if (_ownsHttp) _http.Dispose();
    }
}
