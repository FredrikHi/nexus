-- Let an ingest key create integrations it has never seen.
--
-- Without this, an application cannot report anything until someone has
-- modelled its landscape first, which makes getting started a multi-step
-- ceremony: create the systems, the components and the integrations, then
-- carry their identifiers back into the code.
--
-- Off by default, deliberately. Against an established landscape a misspelled
-- slug should be a loud refusal, not a new row nobody meant to create. Turn it
-- on for the key an application uses while it is being instrumented, and off
-- again once the picture is complete.
ALTER TABLE api_keys
  ADD COLUMN allow_auto_create BOOLEAN NOT NULL DEFAULT false;

COMMENT ON COLUMN api_keys.allow_auto_create IS
  'When true, telemetry naming an unknown integration slug creates that integration.';
