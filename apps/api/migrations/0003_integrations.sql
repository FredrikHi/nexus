CREATE TABLE integrations (
  id                       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id          UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  name                     TEXT NOT NULL,
  slug                     TEXT NOT NULL,
  description              TEXT,
  source_component_id      UUID NOT NULL REFERENCES components(id) ON DELETE CASCADE,
  destination_component_id UUID NOT NULL REFERENCES components(id) ON DELETE CASCADE,
  integration_type_id      UUID NOT NULL REFERENCES integration_types(id),
  environment_id           UUID REFERENCES environments(id) ON DELETE SET NULL,
  owner_team_id            UUID REFERENCES teams(id) ON DELETE SET NULL,
  criticality              TEXT NOT NULL DEFAULT 'MEDIUM'
                             CHECK (criticality IN ('LOW','MEDIUM','HIGH','CRITICAL')),
  status                   TEXT NOT NULL DEFAULT 'ACTIVE'
                             CHECK (status IN ('ACTIVE','INACTIVE','DEPRECATED')),
  documentation_url        TEXT,
  monitoring_enabled       BOOLEAN NOT NULL DEFAULT true,
  health_check_enabled     BOOLEAN NOT NULL DEFAULT false,
  metadata                 JSONB NOT NULL DEFAULT '{}'::jsonb,
  created_at               TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at               TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE (organization_id, slug)
);
CREATE TRIGGER integrations_set_updated_at
  BEFORE UPDATE ON integrations
  FOR EACH ROW EXECUTE FUNCTION set_updated_at();
CREATE INDEX idx_integrations_org ON integrations(organization_id);
CREATE INDEX idx_integrations_source ON integrations(source_component_id);
CREATE INDEX idx_integrations_destination ON integrations(destination_component_id);
CREATE INDEX idx_integrations_env ON integrations(environment_id);
CREATE INDEX idx_integrations_type ON integrations(integration_type_id);

CREATE TABLE dependencies (
  id                       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  source_component_id      UUID NOT NULL REFERENCES components(id) ON DELETE CASCADE,
  destination_component_id UUID NOT NULL REFERENCES components(id) ON DELETE CASCADE,
  dependency_type          TEXT NOT NULL DEFAULT 'RUNTIME'
                             CHECK (dependency_type IN ('RUNTIME','BUILD','DATA','OPTIONAL')),
  description              TEXT,
  created_at               TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE (source_component_id, destination_component_id, dependency_type)
);
CREATE INDEX idx_dependencies_source ON dependencies(source_component_id);
CREATE INDEX idx_dependencies_destination ON dependencies(destination_component_id);

CREATE TABLE tags (
  id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  name            TEXT NOT NULL,
  created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE (organization_id, name)
);
CREATE TABLE system_tags (
  system_id UUID NOT NULL REFERENCES systems(id) ON DELETE CASCADE,
  tag_id    UUID NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
  PRIMARY KEY (system_id, tag_id)
);
CREATE TABLE component_tags (
  component_id UUID NOT NULL REFERENCES components(id) ON DELETE CASCADE,
  tag_id       UUID NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
  PRIMARY KEY (component_id, tag_id)
);
CREATE TABLE integration_tags (
  integration_id UUID NOT NULL REFERENCES integrations(id) ON DELETE CASCADE,
  tag_id         UUID NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
  PRIMARY KEY (integration_id, tag_id)
);
