-- Identity becomes global, and membership becomes many-to-many.
--
-- Until now a user belonged to exactly one organization, which made
-- `organization_id` a column on `users`. That cannot express one person who is
-- in both their own projects and their employer's, so membership moves to its
-- own table and `users` becomes a plain identity.
--
-- Authentication itself lives in a separate Better Auth service. This database
-- never reads that service's tables: it only stores the stable subject id that
-- its signed tokens carry, so identity can be re-pointed at another provider
-- without touching the domain.

ALTER TABLE users DROP COLUMN organization_id;
ALTER TABLE users DROP COLUMN role;

-- The `sub` claim from the auth service. Stable for the life of the account,
-- and the only link between a token and a row here.
ALTER TABLE users ADD COLUMN subject TEXT NOT NULL UNIQUE;
ALTER TABLE users ADD COLUMN avatar_url TEXT;
ALTER TABLE users ADD CONSTRAINT users_email_unique UNIQUE (email);

-- Who belongs to which organization, and with what authority.
--
-- OWNER is distinct from ADMIN so an organization can never be left without
-- someone able to manage it; the service refuses to remove the last owner.
CREATE TABLE organization_members (
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  role            TEXT NOT NULL DEFAULT 'VIEWER'
                    CHECK (role IN ('OWNER','ADMIN','EDITOR','OPERATOR','VIEWER')),
  created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (organization_id, user_id)
);
CREATE TRIGGER organization_members_set_updated_at
  BEFORE UPDATE ON organization_members
  FOR EACH ROW EXECUTE FUNCTION set_updated_at();
-- Listing "my organizations" is the second most common query after resolving
-- the active one, and both start from the user.
CREATE INDEX idx_organization_members_user ON organization_members(user_id);

-- Organizations gain a slug, so the UI can address them in a URL without
-- exposing a UUID, and a description of how their members must authenticate.
ALTER TABLE organizations ADD COLUMN slug TEXT;
UPDATE organizations SET slug = 'default' WHERE slug IS NULL;
ALTER TABLE organizations ALTER COLUMN slug SET NOT NULL;
ALTER TABLE organizations ADD CONSTRAINT organizations_slug_unique UNIQUE (slug);
