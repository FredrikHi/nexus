using System.Text.Json.Serialization;

namespace Nexus.Observability;

/// <summary>How a call across an integration turned out.</summary>
public enum TelemetryStatus
{
    /// <summary>The call returned.</summary>
    Success,

    /// <summary>The dependency failed: a 5xx, or anything else thrown.</summary>
    Failure,

    /// <summary>The call did not come back in time.</summary>
    Timeout,

    /// <summary>
    /// The dependency worked and refused the request: a 4xx, including 429.
    /// Kept separate from <see cref="Failure"/> on purpose. A rate limit is not
    /// an outage, and counting it as one makes a noisy client look like a
    /// broken dependency.
    /// </summary>
    Rejected,
}

/// <summary>One call across one integration.</summary>
public sealed class TelemetryEvent
{
    /// <summary>
    /// The integration's slug, for example <c>orders-to-stripe</c>. Prefer this
    /// over <see cref="IntegrationId"/>: a slug is the same in every instance,
    /// so one build reports to a laptop and to production unchanged.
    /// </summary>
    public string? Integration { get; set; }

    /// <summary>The integration's id. Accepted, but the slug is better.</summary>
    public string? IntegrationId { get; set; }

    public TelemetryStatus Status { get; set; }

    /// <summary>Defaults to the moment the event was recorded.</summary>
    public DateTimeOffset? OccurredAt { get; set; }

    public int? DurationMs { get; set; }

    /// <summary>Events sharing one of these become a trace.</summary>
    public string? TraceId { get; set; }

    /// <summary>What was called, for example <c>POST /charges</c>.</summary>
    public string? Operation { get; set; }

    public int? StatusCode { get; set; }
    public string? ErrorType { get; set; }
    public string? ErrorMessage { get; set; }
    public long? PayloadBytes { get; set; }

    /// <summary>Ignored when the API key is pinned to an environment.</summary>
    public string? EnvironmentId { get; set; }

    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public Dictionary<string, object?>? Metadata { get; set; }
}
