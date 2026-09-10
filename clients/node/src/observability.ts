/**
 * Client for the Integration Observability Platform.
 *
 * Zero dependencies, one file. Copy it into your project or import the
 * package; both work, because the only things it needs are `fetch` and
 * `AsyncLocalStorage`, which Node has had since 18.
 *
 * Three rules shape everything here:
 *
 *  1. It must never break the host application. Every path swallows its own
 *     errors, and a slow or dead collector is invisible to the request that
 *     produced the event.
 *  2. It must never slow a request down. Events queue in memory and flush on a
 *     timer, so recording one costs an array push.
 *  3. Losing telemetry beats degrading the thing being observed. When the
 *     queue fills, the oldest events are dropped and counted.
 */

import { AsyncLocalStorage } from 'node:async_hooks'

export type TelemetryStatus = 'SUCCESS' | 'FAILURE' | 'TIMEOUT' | 'REJECTED'

export interface TelemetryEvent {
  integration_id: string
  status: TelemetryStatus
  occurred_at?: string
  duration_ms?: number
  trace_id?: string
  operation?: string
  status_code?: number
  error_type?: string
  error_message?: string
  payload_bytes?: number
  environment_id?: string
  metadata?: Record<string, unknown>
}

export interface ObservabilityOptions {
  /** Base URL of the API, e.g. http://localhost:8080 */
  url: string
  /** An API key from Administration. It can ingest and nothing else. */
  apiKey: string
  /** Set false to make every call a no-op. Useful in tests and local runs. */
  enabled?: boolean
  /** How often to flush. Lower means fresher health and more requests. */
  flushIntervalMs?: number
  /** Events per request. The server refuses batches over 1000. */
  maxBatch?: number
  /** Hard ceiling on queued events. Beyond this the oldest are dropped. */
  maxQueue?: number
  /** Called when a flush fails. Log it; do not throw from here. */
  onError?: (error: unknown) => void
}

const DEFAULTS = {
  enabled: true,
  flushIntervalMs: 10_000,
  maxBatch: 500,
  maxQueue: 10_000,
}

/**
 * Carries the trace id through async calls without threading it through every
 * function signature. Set it once per request and every tracked call beneath
 * it correlates automatically.
 */
const traceStorage = new AsyncLocalStorage<{ traceId: string }>()

/** The trace id in scope, if any. */
export function currentTraceId(): string | undefined {
  return traceStorage.getStore()?.traceId
}

/**
 * Turns a thrown value into a status and the fields worth keeping.
 *
 * The distinction that matters is FAILURE against REJECTED. A 4xx means the
 * caller sent something wrong, and counting that as an outage would make a
 * noisy client look like a broken dependency.
 */
export function classify(error: unknown): {
  status: TelemetryStatus
  status_code?: number
  error_type?: string
  error_message?: string
} {
  const err = error as {
    name?: string
    code?: string
    message?: string
    status?: number
    statusCode?: number
    response?: { status?: number }
  }

  const statusCode = err?.status ?? err?.statusCode ?? err?.response?.status
  const errorType = err?.name ?? err?.code ?? 'Error'
  const message = typeof err?.message === 'string' ? err.message.slice(0, 500) : undefined

  const timedOut =
    err?.name === 'AbortError' ||
    err?.name === 'TimeoutError' ||
    err?.code === 'ETIMEDOUT' ||
    err?.code === 'ECONNABORTED' ||
    err?.code === 'UND_ERR_HEADERS_TIMEOUT' ||
    statusCode === 408 ||
    statusCode === 504

  if (timedOut) {
    return {
      status: 'TIMEOUT',
      status_code: statusCode,
      error_type: errorType,
      error_message: message,
    }
  }

  // 429 lands here too: rate limiting is a refusal, not a fault. The
  // dependency is working and telling you to slow down.
  if (statusCode !== undefined && statusCode >= 400 && statusCode < 500) {
    return {
      status: 'REJECTED',
      status_code: statusCode,
      error_type: errorType,
      error_message: message,
    }
  }

  return {
    status: 'FAILURE',
    status_code: statusCode,
    error_type: errorType,
    error_message: message,
  }
}

export class Observability {
  private queue: TelemetryEvent[] = []
  private timer: ReturnType<typeof setInterval> | null = null
  private flushing = false
  private dropped = 0
  private readonly options: typeof DEFAULTS & {
    url: string
    apiKey: string
    onError?: (error: unknown) => void
  }

  constructor(options: ObservabilityOptions) {
    this.options = { ...DEFAULTS, ...options }
    if (!this.options.enabled) return

    this.timer = setInterval(() => void this.flush(), this.options.flushIntervalMs)
    // Do not hold the process open just because telemetry is pending.
    this.timer.unref?.()
  }

  /** Queues one event. Returns immediately and never throws. */
  record(event: TelemetryEvent): void {
    if (!this.options.enabled) return

    if (this.queue.length >= this.options.maxQueue) {
      // Drop the oldest rather than the newest: recent events describe what is
      // happening now, which is what health is judged on.
      this.queue.shift()
      this.dropped += 1
      return
    }

    this.queue.push({
      occurred_at: new Date().toISOString(),
      trace_id: currentTraceId(),
      ...event,
    })

    if (this.queue.length >= this.options.maxBatch) void this.flush()
  }

  /**
   * Times a call, records how it went, and returns whatever it returned.
   *
   * Errors are recorded and then rethrown unchanged, so wrapping a call never
   * alters the behaviour of the code around it.
   */
  async track<T>(
    integrationId: string,
    fn: () => Promise<T>,
    options: { operation?: string; metadata?: Record<string, unknown> } = {},
  ): Promise<T> {
    const startedAt = Date.now()
    const occurredAt = new Date().toISOString()

    try {
      const result = await fn()
      this.record({
        integration_id: integrationId,
        status: 'SUCCESS',
        occurred_at: occurredAt,
        duration_ms: Date.now() - startedAt,
        operation: options.operation,
        metadata: options.metadata,
      })
      return result
    } catch (error) {
      this.record({
        integration_id: integrationId,
        occurred_at: occurredAt,
        duration_ms: Date.now() - startedAt,
        operation: options.operation,
        metadata: options.metadata,
        ...classify(error),
      })
      throw error
    }
  }

  /** Runs `fn` with a trace id, so every tracked call inside it correlates. */
  withTrace<T>(traceId: string, fn: () => T): T {
    return traceStorage.run({ traceId }, fn)
  }

  /** Sends whatever is queued. Safe to call at any time. */
  async flush(): Promise<void> {
    if (!this.options.enabled || this.flushing || this.queue.length === 0) return

    this.flushing = true
    try {
      while (this.queue.length > 0) {
        const batch = this.queue.splice(0, this.options.maxBatch)
        try {
          await this.send(batch)
        } catch (error) {
          // Put them back at the front so ordering survives a blip, unless
          // that would overflow the queue, in which case they are lost.
          if (this.queue.length + batch.length <= this.options.maxQueue) {
            this.queue.unshift(...batch)
          } else {
            this.dropped += batch.length
          }
          this.options.onError?.(error)
          break
        }
      }
    } finally {
      this.flushing = false
    }
  }

  /** Flushes and stops the timer. Call this on shutdown. */
  async close(): Promise<void> {
    if (this.timer) clearInterval(this.timer)
    this.timer = null
    await this.flush()
  }

  /** How many events have been dropped. Worth logging periodically. */
  droppedCount(): number {
    return this.dropped
  }

  private async send(events: TelemetryEvent[]): Promise<void> {
    // A slow collector must not become a slow shutdown.
    const controller = new AbortController()
    const timeout = setTimeout(() => controller.abort(), 10_000)

    try {
      const response = await fetch(`${this.options.url}/api/v1/telemetry`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          Authorization: `Bearer ${this.options.apiKey}`,
        },
        body: JSON.stringify({ events }),
        signal: controller.signal,
      })

      if (!response.ok) {
        const body = await response.text().catch(() => '')
        throw new Error(
          `telemetry ingest returned ${response.status}: ${body.slice(0, 200)}`,
        )
      }
    } finally {
      clearTimeout(timeout)
    }
  }
}

/**
 * Process-wide instance, so call sites do not have to pass it around.
 *
 * Left disabled until `initObservability` runs, which means importing this
 * module in a test or a script costs nothing and sends nothing.
 */
export let observability = new Observability({ url: '', apiKey: '', enabled: false })

export function initObservability(options: ObservabilityOptions): Observability {
  observability = new Observability(options)
  return observability
}
