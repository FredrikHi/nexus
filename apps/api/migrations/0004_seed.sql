-- Reference/bootstrap data only. Idempotent so re-running is safe.
-- (Realistic demo systems/telemetry come much later, in the simulation phase.)

INSERT INTO organizations (id, name, description)
VALUES ('00000000-0000-0000-0000-000000000001', 'Default Organization', 'Bootstrap organization')
ON CONFLICT (id) DO NOTHING;

INSERT INTO environments (organization_id, name, slug, is_production) VALUES
  ('00000000-0000-0000-0000-000000000001', 'Development', 'development', false),
  ('00000000-0000-0000-0000-000000000001', 'Test',        'test',        false),
  ('00000000-0000-0000-0000-000000000001', 'Staging',     'staging',     false),
  ('00000000-0000-0000-0000-000000000001', 'Production',  'production',  true)
ON CONFLICT (organization_id, slug) DO NOTHING;

INSERT INTO integration_types (key, name, is_builtin) VALUES
  ('HTTP','HTTP',true),
  ('REST_API','REST API',true),
  ('SOAP','SOAP',true),
  ('SQL','SQL',true),
  ('STORED_PROCEDURE','Stored Procedure',true),
  ('SMTP','SMTP',true),
  ('WEBHOOK','Webhook',true),
  ('MESSAGE_QUEUE','Message Queue',true),
  ('SFTP','SFTP',true),
  ('FILE','File',true),
  ('GRPC','gRPC',true),
  ('INTERNAL','Internal',true),
  ('EXTERNAL','External',true),
  ('OTHER','Other',true)
ON CONFLICT (key) DO NOTHING;
