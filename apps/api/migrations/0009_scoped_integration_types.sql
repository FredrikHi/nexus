-- Integration types become per-organization, with the built-ins shared.
--
-- The catalogue was global, which was fine while there was one tenant. Now a
-- custom type added by one organization would appear in every other one, which
-- leaks both the name and the fact that they use it.
--
-- NULL organization_id means a built-in, visible to everyone. A set one means
-- the type belongs to that organization alone.
ALTER TABLE integration_types
  ADD COLUMN organization_id UUID REFERENCES organizations(id) ON DELETE CASCADE;

-- The old constraint made `key` unique across the whole platform, which would
-- stop two organizations both defining "PEPPOL". Two partial indexes say what
-- was actually meant: built-in keys are unique globally, custom keys are
-- unique within their organization.
ALTER TABLE integration_types DROP CONSTRAINT integration_types_key_key;

CREATE UNIQUE INDEX ux_integration_types_builtin_key
  ON integration_types(key) WHERE organization_id IS NULL;
CREATE UNIQUE INDEX ux_integration_types_org_key
  ON integration_types(organization_id, key) WHERE organization_id IS NOT NULL;

CREATE INDEX idx_integration_types_org ON integration_types(organization_id);

-- A custom type must belong to someone; a built-in must not.
ALTER TABLE integration_types
  ADD CONSTRAINT integration_types_ownership
  CHECK ((is_builtin AND organization_id IS NULL) OR (NOT is_builtin AND organization_id IS NOT NULL));
