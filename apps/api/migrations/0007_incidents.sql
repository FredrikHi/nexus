-- Incidents: what is currently wrong, and what was wrong before.
--
-- Opened and resolved by the reconciler, never by hand. An incident is open
-- while its integration is unhealthy and resolves when it recovers, so a
-- manual close would either be reopened on the next pass or hide a live
-- problem. Humans acknowledge and annotate instead.
CREATE TABLE incidents (
  id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id         UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  integration_id          UUID NOT NULL REFERENCES integrations(id) ON DELETE CASCADE,
  -- ACKNOWLEDGED still means open: someone is looking, nothing is fixed.
  status                  TEXT NOT NULL DEFAULT 'OPEN'
                            CHECK (status IN ('OPEN','ACKNOWLEDGED','RESOLVED')),
  -- Peak severity, never lowered while the incident is open: what mattered
  -- most is the useful number afterwards.
  severity                TEXT NOT NULL
                            CHECK (severity IN ('MINOR','MAJOR','CRITICAL')),
  title                   TEXT NOT NULL,
  -- The evaluator's most recent explanation, in words.
  summary                 TEXT NOT NULL,
  opened_at               TIMESTAMPTZ NOT NULL DEFAULT now(),
  acknowledged_at         TIMESTAMPTZ,
  -- Free text until user authentication lands, then a real user reference.
  acknowledged_by         TEXT,
  resolved_at             TIMESTAMPTZ,
  -- Health at the moment it opened, kept so the incident still means
  -- something after the telemetry behind it ages out of retention.
  opened_error_rate       DOUBLE PRECISION,
  opened_event_count      BIGINT,
  created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
  CHECK ((status = 'RESOLVED') = (resolved_at IS NOT NULL))
);
CREATE TRIGGER incidents_set_updated_at
  BEFORE UPDATE ON incidents
  FOR EACH ROW EXECUTE FUNCTION set_updated_at();

-- The constraint that makes reconciliation safe: at most one unresolved
-- incident per integration. Enforced by the database, so even a race between
-- two reconcilers cannot produce duplicates. A plain UNIQUE would forbid
-- historical incidents too, which is why this is partial.
CREATE UNIQUE INDEX ux_incidents_one_open_per_integration
  ON incidents(integration_id) WHERE resolved_at IS NULL;

CREATE INDEX idx_incidents_org_open
  ON incidents(organization_id, opened_at DESC) WHERE resolved_at IS NULL;
CREATE INDEX idx_incidents_org_time
  ON incidents(organization_id, opened_at DESC);

-- Append-only timeline. Everything that happened to an incident, in order,
-- whether the system did it or a person did.
CREATE TABLE incident_events (
  id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  incident_id UUID NOT NULL REFERENCES incidents(id) ON DELETE CASCADE,
  kind        TEXT NOT NULL
                CHECK (kind IN ('OPENED','ESCALATED','ACKNOWLEDGED','NOTE','RESOLVED')),
  message     TEXT NOT NULL,
  -- NULL means the system did it rather than a person.
  actor       TEXT,
  created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_incident_events_incident
  ON incident_events(incident_id, created_at ASC);
