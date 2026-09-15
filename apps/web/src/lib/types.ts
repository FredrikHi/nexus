// Shapes mirrored from the API's OpenAPI document. Kept hand-written rather
// than generated for now: the surface is small and generation is another
// build step to keep in sync.

export type OrgRole = 'VIEWER' | 'OPERATOR' | 'EDITOR' | 'ADMIN' | 'OWNER'

export interface Membership {
  organization_id: string
  organization_name: string
  organization_slug: string
  role: OrgRole
  joined_at: string
}

export interface Me {
  user_id: string
  email: string
  display_name: string
  avatar_url: string | null
  organizations: Membership[]
}

export interface Organization {
  id: string
  name: string
  slug: string
  description: string | null
  created_at: string
}

export type SystemType =
  | 'APPLICATION' | 'API' | 'DATABASE' | 'EXTERNAL_SERVICE'
  | 'MESSAGE_BROKER' | 'IDENTITY_PROVIDER' | 'FILE_SERVICE' | 'OTHER'

export type Criticality = 'LOW' | 'MEDIUM' | 'HIGH' | 'CRITICAL'
export type LifecycleStatus = 'PLANNED' | 'ACTIVE' | 'DEPRECATED' | 'RETIRED'

export interface System {
  id: string
  organization_id: string
  name: string
  slug: string
  description: string | null
  system_type: SystemType
  criticality: Criticality
  lifecycle_status: LifecycleStatus
  documentation_url: string | null
  repository_url: string | null
  created_at: string
  updated_at: string
}

export type ComponentType =
  | 'FRONTEND' | 'API' | 'CONTROLLER' | 'SERVICE' | 'REPOSITORY' | 'DATABASE'
  | 'STORED_PROCEDURE' | 'WORKER' | 'JOB' | 'QUEUE' | 'WEBHOOK' | 'OTHER'

export interface Component {
  id: string
  system_id: string
  name: string
  slug: string
  description: string | null
  component_type: ComponentType
  created_at: string
}

export type IntegrationStatus = 'ACTIVE' | 'INACTIVE' | 'DEPRECATED'

export interface Integration {
  id: string
  organization_id: string
  name: string
  slug: string
  description: string | null
  source_component_id: string
  destination_component_id: string
  integration_type_id: string
  environment_id: string | null
  criticality: Criticality
  status: IntegrationStatus
  monitoring_enabled: boolean
  created_at: string
}

export interface IntegrationType {
  id: string
  key: string
  name: string
  description: string | null
  is_builtin: boolean
  /** Null for a built-in; set means it belongs to this organization alone. */
  organization_id: string | null
  created_at: string
}

export interface Environment {
  id: string
  organization_id: string
  name: string
  slug: string
  description: string | null
  is_production: boolean
  created_at: string
  updated_at: string
}

export type HealthStatus = 'HEALTHY' | 'DEGRADED' | 'UNHEALTHY' | 'UNKNOWN'

export interface IntegrationHealth {
  integration_id: string
  integration_name: string
  integration_slug: string
  status: HealthStatus
  since: string
  evaluated_at: string
  window_minutes: number
  event_count: number
  error_count: number
  error_rate: number
  p95_duration_ms: number | null
  last_event_at: string | null
  reason: string
}

export type Severity = 'MINOR' | 'MAJOR' | 'CRITICAL'
export type IncidentStatus = 'OPEN' | 'ACKNOWLEDGED' | 'RESOLVED'

export interface Incident {
  id: string
  integration_id: string
  status: IncidentStatus
  severity: Severity
  title: string
  summary: string
  opened_at: string
  acknowledged_at: string | null
  acknowledged_by: string | null
  resolved_at: string | null
}

export interface TelemetrySummary {
  total: number
  success: number
  failure: number
  timeout: number
  rejected: number
  error_rate: number
  p50_duration_ms: number | null
  p95_duration_ms: number | null
  p99_duration_ms: number | null
}

export type TelemetryStatus = 'SUCCESS' | 'FAILURE' | 'TIMEOUT' | 'REJECTED'

export interface TelemetryEvent {
  id: string
  integration_id: string
  environment_id: string | null
  occurred_at: string
  status: TelemetryStatus
  duration_ms: number | null
  trace_id: string | null
  operation: string | null
  status_code: number | null
  error_type: string | null
  error_message: string | null
  received_at: string
}

export interface SeriesPoint {
  bucket: string
  total: number
  success: number
  failure: number
  timeout: number
  rejected: number
  error_rate: number
  p95_duration_ms: number | null
}

export interface TraceSummary {
  trace_id: string
  started_at: string
  ended_at: string
  elapsed_ms: number
  span_count: number
  integration_count: number
  error_count: number
  status: TelemetryStatus
}

export interface TraceDetail extends TraceSummary {
  spans: TelemetryEvent[]
}

export interface Team {
  id: string
  organization_id: string
  name: string
  slug: string
  description: string | null
  member_count: number
  owned_systems: number
  owned_integrations: number
  created_at: string
}

export interface TeamMember {
  user_id: string
  email: string
  display_name: string
  avatar_url: string | null
}

export interface Member {
  user_id: string
  email: string
  display_name: string
  avatar_url: string | null
  role: OrgRole
  joined_at: string
}

export interface ApiKey {
  id: string
  organization_id: string
  name: string
  /** Leading, non-secret slice of the token, so two keys can be told apart. */
  prefix: string
  environment_id: string | null
  allow_auto_create: boolean
  last_used_at: string | null
  expires_at: string | null
  revoked_at: string | null
  created_at: string
  updated_at: string
}

/** The create response, and the only time the plaintext token exists. */
export interface CreatedApiKey extends ApiKey {
  token: string
}
