//! Carries a child process's output into this process's log.

use std::borrow::Cow;

use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tracing::Level;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stream {
    Stdout,
    Stderr,
}

/// How a child writes its log lines, so each can be re-logged at the level it
/// was written at, without a second timestamp in front of our own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineFormat {
    /// No structure: stdout is info, stderr is a warning.
    Plain,
    /// Rust's `tracing` formatter, as the API writes:
    /// `2026-09-17T14:28:00.004459Z  INFO integration_api: migrations applied`
    Tracing,
    /// PostgreSQL's default `log_line_prefix`, written to stderr whatever the
    /// level: `2026-09-17 16:27:56.864 CEST [28204] LOG:  database system is ready`
    Postgres,
}

/// Reads `reader` line by line until the process closes it, logging each line
/// tagged with the child's name.
///
/// Reads bytes and converts lossily rather than reading `String` lines. A line
/// that is not valid UTF-8 (a Windows code page, a stack trace with a stray
/// byte) would otherwise end the loop, nobody would drain the pipe any more,
/// and once its buffer filled the child would block on its next write: an API
/// that hangs because of a log line.
pub fn forward(
    child: String,
    reader: impl AsyncRead + Unpin + Send + 'static,
    stream: Stream,
    format: LineFormat,
) {
    tokio::spawn(async move {
        let mut reader = BufReader::new(reader);
        let mut line = Vec::new();
        loop {
            line.clear();
            match reader.read_until(b'\n', &mut line).await {
                // End of stream: the process has exited or closed its output.
                Ok(0) => break,
                Ok(_) => {
                    let text = String::from_utf8_lossy(&line);
                    let text = text.trim_end();
                    if text.is_empty() {
                        continue;
                    }
                    let (level, message) = classify(text, stream, format);
                    emit(level, &child, &message);
                }
                Err(err) => {
                    tracing::debug!(child = %child, error = %err, "stopped reading output");
                    break;
                }
            }
        }
    });
}

/// `tracing` only has macros with the level fixed at compile time, so a level
/// chosen at run time picks its macro here.
fn emit(level: Level, child: &str, message: &str) {
    match level {
        Level::ERROR => tracing::error!(child = %child, "{message}"),
        Level::WARN => tracing::warn!(child = %child, "{message}"),
        Level::INFO => tracing::info!(child = %child, "{message}"),
        _ => tracing::debug!(child = %child, "{message}"),
    }
}

/// The level a line was written at, and the line without the parts our own
/// log line already has. A line that does not match its format is logged
/// whole, as a plain line: better a doubled timestamp than a lost message.
pub fn classify(line: &str, stream: Stream, format: LineFormat) -> (Level, Cow<'_, str>) {
    let parsed = match format {
        LineFormat::Plain => None,
        LineFormat::Tracing => tracing_line(line),
        LineFormat::Postgres => postgres_line(line),
    };
    parsed.unwrap_or_else(|| {
        let level = match stream {
            Stream::Stdout => Level::INFO,
            Stream::Stderr => Level::WARN,
        };
        (level, Cow::Borrowed(line))
    })
}

/// `<timestamp> <LEVEL> <target>: <message>`, keeping `target: message`.
fn tracing_line(line: &str) -> Option<(Level, Cow<'_, str>)> {
    let (timestamp, rest) = line.split_once(' ')?;
    if !(timestamp.contains('T') && timestamp.ends_with('Z')) {
        return None;
    }
    let rest = rest.trim_start();
    let (level, message) = rest.split_once(' ')?;
    let level = match level {
        "ERROR" => Level::ERROR,
        "WARN" => Level::WARN,
        "INFO" => Level::INFO,
        "DEBUG" => Level::DEBUG,
        "TRACE" => Level::TRACE,
        _ => return None,
    };
    Some((level, Cow::Borrowed(message.trim_start())))
}

/// `<date> <time> <zone> [<pid>] <LEVEL>:  <message>`, keeping `[pid] message`:
/// the process id is how one connection's lines are told from another's.
fn postgres_line(line: &str) -> Option<(Level, Cow<'_, str>)> {
    let open = line.find(" [")?;
    let close = open + line[open..].find("] ")?;
    let pid = &line[open + 1..=close];
    let (level, message) = line[close + 2..].split_once(':')?;
    let level = match level {
        "PANIC" | "FATAL" | "ERROR" => Level::ERROR,
        "WARNING" => Level::WARN,
        // DETAIL, HINT, CONTEXT and STATEMENT elaborate on the line before;
        // info keeps them next to it in any filter that shows the line.
        "LOG" | "INFO" | "NOTICE" | "DETAIL" | "HINT" | "CONTEXT" | "STATEMENT" => Level::INFO,
        level if level.starts_with("DEBUG") => Level::DEBUG,
        _ => return None,
    };
    Some((level, Cow::Owned(format!("{pid} {}", message.trim_start()))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_lines_keep_their_level_and_lose_their_timestamp() {
        let (level, message) = classify(
            "2026-09-17T14:28:00.004459Z  INFO integration_api: database migrations applied",
            Stream::Stdout,
            LineFormat::Tracing,
        );
        assert_eq!(level, Level::INFO);
        assert_eq!(message, "integration_api: database migrations applied");

        let (level, _) = classify(
            "2026-09-17T14:28:00.004459Z ERROR integration_api: database unreachable",
            Stream::Stdout,
            LineFormat::Tracing,
        );
        assert_eq!(level, Level::ERROR);
    }

    #[test]
    fn postgres_log_lines_on_stderr_are_not_warnings() {
        let (level, message) = classify(
            "2026-09-17 16:27:56.864 CEST [28204] LOG:  database system is ready to accept connections",
            Stream::Stderr,
            LineFormat::Postgres,
        );
        assert_eq!(level, Level::INFO);
        assert_eq!(
            message,
            "[28204] database system is ready to accept connections"
        );
    }

    #[test]
    fn postgres_failures_are_errors_and_warnings_are_warnings() {
        let fatal = "2026-09-17 16:28:58.320 CEST [52588] FATAL:  password authentication failed for user \"postgres\"";
        let warning =
            "2026-09-17 16:28:58.320 CEST [52588] WARNING:  there is no transaction in progress";

        assert_eq!(
            classify(fatal, Stream::Stderr, LineFormat::Postgres).0,
            Level::ERROR
        );
        assert_eq!(
            classify(warning, Stream::Stderr, LineFormat::Postgres).0,
            Level::WARN
        );
    }

    #[test]
    fn a_line_that_does_not_fit_its_format_is_kept_whole() {
        // A panic message, or anything printed before logging was set up.
        let line = "thread 'main' panicked at src/main.rs:1:1";

        let (level, message) = classify(line, Stream::Stderr, LineFormat::Tracing);
        assert_eq!(level, Level::WARN);
        assert_eq!(message, line);

        let (level, message) = classify(line, Stream::Stdout, LineFormat::Postgres);
        assert_eq!(level, Level::INFO);
        assert_eq!(message, line);
    }

    #[test]
    fn plain_output_goes_by_stream() {
        assert_eq!(
            classify("auth migrations applied", Stream::Stdout, LineFormat::Plain).0,
            Level::INFO
        );
        assert_eq!(
            classify(
                "auth migrations failed: boom",
                Stream::Stderr,
                LineFormat::Plain
            )
            .0,
            Level::WARN
        );
    }
}
