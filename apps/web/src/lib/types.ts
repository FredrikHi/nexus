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
}

export interface Environment {
  id: string
  name: string
  slug: string
  is_production: boolean
}

export type HealthStatus = 'HEALTHY' | 'DEGRADED' | 'UNHEALTHY' | 'UNKNOWN'

export interface IntegrationHealth {
  integration_id: string
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
