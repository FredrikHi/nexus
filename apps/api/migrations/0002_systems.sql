CREATE TABLE systems (
  id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id   UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  name              TEXT NOT NULL,
  slug              TEXT NOT NULL,
  description       TEXT,
  system_type       TEXT NOT NULL DEFAULT 'APPLICATION'
                      CHECK (system_type IN ('APPLICATION','API','DATABASE','EXTERNAL_SERVICE',
                                             'MESSAGE_BROKER','IDENTITY_PROVIDER','FILE_SERVICE','OTHER')),
  owner_team_id     UUID REFERENCES teams(id) ON DELETE SET NULL,
  documentation_url TEXT,
  repository_url    TEXT,
  criticality       TEXT NOT NULL DEFAULT 'MEDIUM'
                      CHECK (criticality IN ('LOW','MEDIUM','HIGH','CRITICAL')),
  lifecycle_status  TEXT NOT NULL DEFAULT 'ACTIVE'
                      CHECK (lifecycle_status IN ('PLANNED','ACTIVE','DEPRECATED','RETIRED')),
  metadata          JSONB NOT NULL DEFAULT '{}'::jsonb,
  created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE (organization_id, slug)
);
CREATE TRIGGER systems_set_updated_at
  BEFORE UPDATE ON systems
  FOR EACH ROW EXECUTE FUNCTION set_updated_at();
CREATE INDEX idx_systems_org ON systems(organization_id);
CREATE INDEX idx_systems_type ON systems(system_type);

CREATE TABLE components (
  id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  system_id         UUID NOT NULL REFERENCES systems(id) ON DELETE CASCADE,
  name              TEXT NOT NULL,
  slug              TEXT NOT NULL,
  description       TEXT,
  component_type    TEXT NOT NULL DEFAULT 'OTHER'
                      CHECK (component_type IN ('FRONTEND','API','CONTROLLER','SERVICE','REPOSITORY',
                                                'DATABASE','STORED_PROCEDURE','WORKER','JOB','QUEUE',
                                                'WEBHOOK','OTHER')),
  owner_team_id     UUID REFERENCES teams(id) ON DELETE SET NULL,
  repository_url    TEXT,
  documentation_url TEXT,
  metadata          JSONB NOT NULL DEFAULT '{}'::jsonb,
  created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE (system_id, slug)
);
CREATE TRIGGER components_set_updated_at
  BEFORE UPDATE ON components
  FOR EACH ROW EXECUTE FUNCTION set_updated_at();
CREATE INDEX idx_components_system ON components(system_id);

CREATE TABLE integration_types (
  id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  key         TEXT NOT NULL UNIQUE,
  name        TEXT NOT NULL,
  description TEXT,
  is_builtin  BOOLEAN NOT NULL DEFAULT false,
  created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
