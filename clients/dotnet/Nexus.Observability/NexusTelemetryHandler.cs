using System.Diagnostics;

namespace Nexus.Observability;

/// <summary>
/// Records every call an <see cref="HttpClient"/> makes, without touching a
/// single call site.
///
/// This is the .NET equivalent of subscribing to an SDK's own response event:
/// register it once on a typed client and every request through that client is
/// reported, including ones added later by someone who has never heard of
/// Nexus.
/// </summary>
public sealed class NexusTelemetryHandler : DelegatingHandler
{
    /// <summary>
    /// Set this on a request to report it against a different integration than
    /// the handler's default. Useful when one client serves several.
    /// </summary>
    public static readonly HttpRequestOptionsKey<string> IntegrationOption = new("Nexus.Integration");

    private readonly NexusClient _client;
    private readonly string _integration;

    public NexusTelemetryHandler(NexusClient client, string integration)
    {
        _client = client ?? throw new ArgumentNullException(nameof(client));
        _integration = integration ?? throw new ArgumentNullException(nameof(integration));
    }

    protected override async Task<HttpResponseMessage> SendAsync(
        HttpRequestMessage request,
        CancellationToken cancellationToken)
    {
        var integration = request.Options.TryGetValue(IntegrationOption, out var named)
            ? named
            : _integration;

        // The path without its query string: a query carries ids and secrets,
        // and would make every request its own operation name.
        var operation = $"{request.Method.Method} {request.RequestUri?.AbsolutePath}";

        var occurredAt = DateTimeOffset.UtcNow;
        var started = Stopwatch.GetTimestamp();

        try
        {
            var response = await base.SendAsync(request, cancellationToken).ConfigureAwait(false);

            // A 500 does not throw here, so the status has to be read rather
            // than waited for. Wrapping only the call would record every failed
            // request as a success.
            _client.Record(new TelemetryEvent
            {
                Integration = integration,
                Status = Outcome.FromStatusCode((int)response.StatusCode),
                OccurredAt = occurredAt,
                DurationMs = (int)Stopwatch.GetElapsedTime(started).TotalMilliseconds,
                Operation = operation,
                StatusCode = (int)response.StatusCode,
            });

            return response;
        }
        catch (Exception exception)
        {
            var (status, statusCode, errorType, message) = Outcome.Classify(exception);
            _client.Record(new TelemetryEvent
            {
                Integration = integration,
                Status = status,
                OccurredAt = occurredAt,
                DurationMs = (int)Stopwatch.GetElapsedTime(started).TotalMilliseconds,
                Operation = operation,
                StatusCode = statusCode,
                ErrorType = errorType,
                ErrorMessage = message,
            });
            throw;
        }
    }
}
