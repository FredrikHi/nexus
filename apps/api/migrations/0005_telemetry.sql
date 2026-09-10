-- API keys: how an ingesting client proves which organization it is.
--
-- The raw token is never stored. We keep a SHA-256 hash and look keys up by
-- that hash, so a database leak does not hand out working credentials.
-- SHA-256 rather than argon2/bcrypt on purpose: those are deliberately slow to
-- make low-entropy PASSWORDS expensive to brute force. An API key is 32 bytes
-- of CSPRNG output, so there is nothing to brute force, and this runs on every
-- ingest request where slow would be a denial-of-service vector.
CREATE TABLE api_keys (
  id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  name            TEXT NOT NULL,
  -- Leading chars of the token, safe to display so a human can identify a key.
  prefix          TEXT NOT NULL,
  token_hash      TEXT NOT NULL UNIQUE,
  -- Optional: pin a key to one environment so a staging agent cannot write
  -- events that claim to be production.
  environment_id  UUID REFERENCES environments(id) ON DELETE SET NULL,
  last_used_at    TIMESTAMPTZ,
  expires_at      TIMESTAMPTZ,
  revoked_at      TIMESTAMPTZ,
  created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE (organization_id, name)
);
CREATE TRIGGER api_keys_set_updated_at
  BEFORE UPDATE ON api_keys
  FOR EACH ROW EXECUTE FUNCTION set_updated_at();
CREATE INDEX idx_api_keys_org ON api_keys(organization_id);

-- Telemetry events: one recorded call across one integration.
--
-- RANGE partitioned by month on occurred_at. This is the largest table in the
-- system by orders of magnitude, and expiring old data by DROP TABLE on a
-- partition is effectively instant, where a DELETE over a huge unpartitioned
-- table is a long, bloating transaction.
--
-- Postgres requires the partition key to be part of any primary key, which is
-- why the key is (occurred_at, id) rather than id alone.
CREATE TABLE telemetry_events (
  id                UUID NOT NULL DEFAULT gen_random_uuid(),
  organization_id   UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  integration_id    UUID NOT NULL REFERENCES integrations(id) ON DELETE CASCADE,
  environment_id    UUID REFERENCES environments(id) ON DELETE SET NULL,
  occurred_at       TIMESTAMPTZ NOT NULL,
  -- SUCCESS  the call completed as intended
  -- FAILURE  the callee errored (a 5xx, an exception, a broken pipe)
  -- TIMEOUT  no answer within the caller's budget
  -- REJECTED the callee refused deliberately (auth, validation, a 4xx)
  -- FAILURE and REJECTED are separated because only one of them means the
  -- integration is unhealthy; the other means the caller sent something wrong.
  status            TEXT NOT NULL
                      CHECK (status IN ('SUCCESS','FAILURE','TIMEOUT','REJECTED')),
  duration_ms       INTEGER CHECK (duration_ms IS NULL OR duration_ms >= 0),
  -- Correlation handle. Traces (next phase) group events sharing a trace_id.
  trace_id          TEXT,
  -- What was called, e.g. "POST /invoices" or "SendInvoice".
  operation         TEXT,
  -- Protocol-level code where one exists (HTTP status, SMTP code).
  status_code       INTEGER,
  error_type        TEXT,
  error_message     TEXT,
  payload_bytes     BIGINT CHECK (payload_bytes IS NULL OR payload_bytes >= 0),
  metadata          JSONB NOT NULL DEFAULT '{}'::jsonb,
  received_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (occurred_at, id)
) PARTITION BY RANGE (occurred_at);

-- Indexes declared on the parent propagate to every partition, existing and
-- future. Ordered DESC because every query here asks for the recent end.
CREATE INDEX idx_telemetry_org_time
  ON telemetry_events(organization_id, occurred_at DESC);
CREATE INDEX idx_telemetry_integration_time
  ON telemetry_events(integration_id, occurred_at DESC);
CREATE INDEX idx_telemetry_status_time
  ON telemetry_events(organization_id, status, occurred_at DESC);
CREATE INDEX idx_telemetry_trace
  ON telemetry_events(trace_id) WHERE trace_id IS NOT NULL;

-- Creates the monthly partition covering `target` if it does not exist yet.
-- Idempotent, so it is safe to call on every boot.
--
-- Boundaries are computed in UTC explicitly. date_trunc on a timestamptz
-- otherwise depends on the session TimeZone, which would silently shift where
-- a partition begins depending on who connected.
CREATE OR REPLACE FUNCTION ensure_telemetry_partition(target TIMESTAMPTZ)
RETURNS TEXT AS $$
DECLARE
  start_ts TIMESTAMPTZ := date_trunc('month', target AT TIME ZONE 'UTC') AT TIME ZONE 'UTC';
  end_ts   TIMESTAMPTZ := start_ts + INTERVAL '1 month';
  part     TEXT := 'telemetry_events_' || to_char(start_ts AT TIME ZONE 'UTC', 'YYYY_MM');
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_class WHERE relname = part AND relkind = 'r'
  ) THEN
    -- Dynamic SQL: an identifier cannot be a bind parameter. %I quotes an
    -- identifier and %L a literal, which is what keeps this injection-safe.
    EXECUTE format(
      'CREATE TABLE %I PARTITION OF telemetry_events FOR VALUES FROM (%L) TO (%L)',
      part, start_ts, end_ts
    );
  END IF;
  RETURN part;
END;
$$ LANGUAGE plpgsql;

-- Retention: drop every partition whose whole range is older than `cutoff`.
-- Returns how many were dropped. DROP TABLE on a partition reclaims the space
-- immediately, with no vacuum needed.
CREATE OR REPLACE FUNCTION drop_telemetry_partitions_before(cutoff TIMESTAMPTZ)
RETURNS INTEGER AS $$
DECLARE
  r         RECORD;
  dropped   INTEGER := 0;
  part_end  TIMESTAMPTZ;
BEGIN
  FOR r IN
    SELECT c.relname
    FROM pg_inherits i
    JOIN pg_class c ON c.oid = i.inhrelid
    JOIN pg_class p ON p.oid = i.inhparent
    WHERE p.relname = 'telemetry_events'
  LOOP
    -- The name encodes the month it covers, e.g. telemetry_events_2026_09.
    part_end := (to_date(right(r.relname, 7), 'YYYY_MM') + INTERVAL '1 month')
                AT TIME ZONE 'UTC';
    IF part_end <= cutoff THEN
      EXECUTE format('DROP TABLE %I', r.relname);
      dropped := dropped + 1;
    END IF;
  END LOOP;
  RETURN dropped;
END;
$$ LANGUAGE plpgsql;

-- Seed a window around now so ingest works immediately after migration.
-- The API extends this window on every boot.
DO $$
DECLARE m INTEGER;
BEGIN
  FOR m IN -1..3 LOOP
    PERFORM ensure_telemetry_partition(now() + (m || ' month')::INTERVAL);
  END LOOP;
END $$;
