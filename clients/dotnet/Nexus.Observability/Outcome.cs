using System.Net;

namespace Nexus.Observability;

/// <summary>Turns a thrown exception into a status and the fields worth keeping.</summary>
internal static class Outcome
{
    public static (TelemetryStatus Status, int? StatusCode, string ErrorType, string? Message) Classify(
        Exception exception)
    {
        var statusCode = StatusCodeOf(exception);
        var type = exception.GetType().Name;
        var message = Truncate(exception.Message);

        // A cancelled task caused by a timeout, rather than by a caller
        // deliberately cancelling, is the shape HttpClient uses for its own
        // timeout. Both land here as TIMEOUT because from the dependency's
        // point of view the call never came back.
        if (exception is TimeoutException or OperationCanceledException)
        {
            return (TelemetryStatus.Timeout, statusCode, type, message);
        }

        if (statusCode is (int)HttpStatusCode.RequestTimeout or (int)HttpStatusCode.GatewayTimeout)
        {
            return (TelemetryStatus.Timeout, statusCode, type, message);
        }

        // 429 lands here too: rate limiting is a refusal, not a fault.
        if (statusCode is >= 400 and < 500)
        {
            return (TelemetryStatus.Rejected, statusCode, type, message);
        }

        return (TelemetryStatus.Failure, statusCode, type, message);
    }

    /// <summary>Maps a response's status code, for the handler that sees responses rather than exceptions.</summary>
    public static TelemetryStatus FromStatusCode(int statusCode) => statusCode switch
    {
        (int)HttpStatusCode.RequestTimeout or (int)HttpStatusCode.GatewayTimeout => TelemetryStatus.Timeout,
        >= 400 and < 500 => TelemetryStatus.Rejected,
        >= 500 => TelemetryStatus.Failure,
        _ => TelemetryStatus.Success,
    };

    private static int? StatusCodeOf(Exception exception) => exception switch
    {
        HttpRequestException http when http.StatusCode is not null => (int)http.StatusCode.Value,
        // Anything carrying a Data["StatusCode"], which is how most SDKs that
        // wrap HTTP surface the code without a shared base type.
        _ when exception.Data["StatusCode"] is int code => code,
        _ => null,
    };

    private static string? Truncate(string? message) =>
        message is null ? null : message.Length <= 500 ? message : message[..500];
}
