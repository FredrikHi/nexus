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
            var options = FromEnvironment();
            configure?.Invoke(options);
            return new NexusClient(options);
        });

        return services;
    }

    /// <summary>
    /// Registers the client as a singleton, configured with the container in
    /// hand.
    ///
    /// The overload above covers the common case. Reach for this one when the
    /// settings come from <c>IConfiguration</c> rather than the environment, or
    /// when <see cref="NexusOptions.OnError"/> should log through the
    /// application's logger instead of disappearing:
    ///
    /// <code>
    /// services.AddNexus((provider, options) =>
    /// {
    ///     var logger = provider.GetRequiredService&lt;ILogger&lt;NexusClient&gt;&gt;();
    ///     options.OnError = e => logger.LogWarning(e, "Nexus flush failed");
    /// });
    /// </code>
    /// </summary>
    public static IServiceCollection AddNexus(
        this IServiceCollection services,
        Action<IServiceProvider, NexusOptions> configure)
    {
        ArgumentNullException.ThrowIfNull(configure);

        services.AddSingleton(provider =>
        {
            var options = FromEnvironment();
            configure(provider, options);
            return new NexusClient(options);
        });

        return services;
    }

    // Both overloads start from the environment, so moving from one to the
    // other never silently changes where the URL and key come from.
    private static NexusOptions FromEnvironment() => new()
    {
        Url = Environment.GetEnvironmentVariable("NEXUS_URL") ?? "",
        ApiKey = Environment.GetEnvironmentVariable("NEXUS_API_KEY") ?? "",
    };

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
