using Microsoft.Extensions.DependencyInjection;

namespace Nexus.Observability;

public static class NexusServiceCollectionExtensions
{
    /// <summary>
    /// Registers the client as a singleton.
    ///
    /// Reads NEXUS_URL and NEXUS_API_KEY from the environment first, so a
    /// deployment configures it without a code change, and leaving the key
    /// unset makes the whole thing inert.
    /// </summary>
    public static IServiceCollection AddNexus(
        this IServiceCollection services,
        Action<NexusOptions>? configure = null)
    {
        services.AddSingleton(_ =>
        {
            var options = new NexusOptions
            {
                Url = Environment.GetEnvironmentVariable("NEXUS_URL") ?? "",
                ApiKey = Environment.GetEnvironmentVariable("NEXUS_API_KEY") ?? "",
            };
            configure?.Invoke(options);
            return new NexusClient(options);
        });

        return services;
    }

    /// <summary>
    /// Reports every call this client makes against the named integration.
    ///
    /// <code>
    /// services.AddHttpClient("stripe").AddNexusTelemetry("orders-to-stripe");
    /// </code>
    /// </summary>
    public static IHttpClientBuilder AddNexusTelemetry(
        this IHttpClientBuilder builder,
        string integration)
    {
        return builder.AddHttpMessageHandler(provider =>
            new NexusTelemetryHandler(provider.GetRequiredService<NexusClient>(), integration));
    }
}
