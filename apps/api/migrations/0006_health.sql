-- Thresholds that turn telemetry into a health verdict.
--
-- One row per organization with integration_id NULL is the default; a row with
-- an integration_id overrides it for that integration. Postgres treats NULLs
-- as distinct in a UNIQUE constraint, so "at most one default per org" needs a
-- partial unique index rather than UNIQUE (organization_id, integration_id).
CREATE TABLE health_policies (
  id                   UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id      UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  integration_id       UUID REFERENCES integrations(id) ON DELETE CASCADE,
  -- How far back each evaluation looks.
  window_minutes       INTEGER NOT NULL DEFAULT 15
                         CHECK (window_minutes BETWEEN 1 AND 1440),
  -- Below this many events in the window the verdict stays UNKNOWN, so a
  -- single failure on a quiet integration cannot declare an outage.
  min_events           INTEGER NOT NULL DEFAULT 5 CHECK (min_events >= 0),
  error_rate_degraded  DOUBLE PRECISION NOT NULL DEFAULT 0.05
                         CHECK (error_rate_degraded BETWEEN 0 AND 1),
  error_rate_unhealthy DOUBLE PRECISION NOT NULL DEFAULT 0.20
                         CHECK (error_rate_unhealthy BETWEEN 0 AND 1),
  -- NULL means latency is not part of the verdict for this policy.
  p95_degraded_ms      INTEGER CHECK (p95_degraded_ms IS NULL OR p95_degraded_ms >= 0),
  p95_unhealthy_ms     INTEGER CHECK (p95_unhealthy_ms IS NULL OR p95_unhealthy_ms >= 0),
  -- No events for this long and the verdict returns to UNKNOWN. Silence is
  -- not failure: plenty of integrations are idle by design between runs.
  stale_after_minutes  INTEGER NOT NULL DEFAULT 60 CHECK (stale_after_minutes >= 1),
  created_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
  CHECK (error_rate_degraded <= error_rate_unhealthy),
  CHECK (p95_degraded_ms IS NULL OR p95_unhealthy_ms IS NULL
         OR p95_degraded_ms <= p95_unhealthy_ms)
);
CREATE TRIGGER health_policies_set_updated_at
  BEFORE UPDATE ON health_policies
  FOR EACH ROW EXECUTE FUNCTION set_updated_at();
CREATE UNIQUE INDEX ux_health_policies_org_default
  ON health_policies(organization_id) WHERE integration_id IS NULL;
CREATE UNIQUE INDEX ux_health_policies_integration
  ON health_policies(integration_id) WHERE integration_id IS NOT NULL;

-- Current verdict per integration. One row, overwritten each evaluation.
-- `since` is when the CURRENT status began, which is what an incident needs to
-- know how long something has been broken.
CREATE TABLE integration_health (
  integration_id  UUID PRIMARY KEY REFERENCES integrations(id) ON DELETE CASCADE,
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  status          TEXT NOT NULL
                    CHECK (status IN ('HEALTHY','DEGRADED','UNHEALTHY','UNKNOWN')),
  since           TIMESTAMPTZ NOT NULL DEFAULT now(),
  evaluated_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
  window_minutes  INTEGER NOT NULL,
  event_count     BIGINT NOT NULL,
  error_count     BIGINT NOT NULL,
  error_rate      DOUBLE PRECISION NOT NULL,
  p95_duration_ms DOUBLE PRECISION,
  last_event_at   TIMESTAMPTZ,
  -- Why the evaluator decided what it decided, in words.
  reason          TEXT NOT NULL
);
CREATE INDEX idx_integration_health_org ON integration_health(organization_id, status);

-- Append-only log of every status change. Incidents are built from this: an
-- incident opens on a transition into a bad state and closes on recovery.
CREATE TABLE health_transitions (
  id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  integration_id  UUID NOT NULL REFERENCES integrations(id) ON DELETE CASCADE,
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  -- NULL on the very first evaluation of an integration.
  from_status     TEXT CHECK (from_status IN ('HEALTHY','DEGRADED','UNHEALTHY','UNKNOWN')),
  to_status       TEXT NOT NULL
                    CHECK (to_status IN ('HEALTHY','DEGRADED','UNHEALTHY','UNKNOWN')),
  changed_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
  reason          TEXT NOT NULL,
  error_rate      DOUBLE PRECISION,
  event_count     BIGINT
);
CREATE INDEX idx_health_transitions_integration
  ON health_transitions(integration_id, changed_at DESC);
CREATE INDEX idx_health_transitions_org
  ON health_transitions(organization_id, changed_at DESC);

-- A default policy for the bootstrap organization so evaluation works
-- out of the box.
INSERT INTO health_policies (organization_id, integration_id)
VALUES ('00000000-0000-0000-0000-000000000001', NULL)
ON CONFLICT DO NOTHING;
