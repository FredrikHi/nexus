namespace Nexus.Observability;

/// <summary>How the client talks to Nexus, and how much it is willing to hold.</summary>
public sealed class NexusOptions
{
    /// <summary>Base URL of your Nexus instance, for example https://nexus.example.com.</summary>
    public string Url { get; set; } = "";

    /// <summary>An ingest key. It can write telemetry and read nothing.</summary>
    public string ApiKey { get; set; } = "";

    /// <summary>
    /// Set false to make every call a no-op. Defaults to true only when an API
    /// key is present, so tests and local runs send nothing by accident.
    /// </summary>
    public bool? Enabled { get; set; }

    /// <summary>How often queued events are sent. Lower means fresher health and more requests.</summary>
    public TimeSpan FlushInterval { get; set; } = TimeSpan.FromSeconds(10);

    /// <summary>Events per request. The server refuses batches over 1000.</summary>
    public int MaxBatch { get; set; } = 500;

    /// <summary>
    /// Hard ceiling on queued events. Past this the oldest are dropped, because
    /// recent events describe what is happening now, which is what health is
    /// judged on.
    /// </summary>
    public int MaxQueue { get; set; } = 10_000;

    /// <summary>How long a single send may take before it is abandoned.</summary>
    public TimeSpan SendTimeout { get; set; } = TimeSpan.FromSeconds(10);

    /// <summary>Called when a flush fails. Log it; do not throw from here.</summary>
    public Action<Exception>? OnError { get; set; }

    internal bool IsEnabled => Enabled ?? !string.IsNullOrWhiteSpace(ApiKey);
}
