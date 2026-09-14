#!/usr/bin/env node
/**
 * Creates or updates a landscape from a JSON file, then prints the integration
 * ids your application needs.
 *
 * Idempotent: it matches existing records by name and only creates what is
 * missing, so re-running after editing the file adds the new parts and leaves
 * the rest alone. That makes the landscape something you keep in version
 * control next to the code it describes, rather than clicking together once.
 *
 * Usage:
 *   node scripts/apply-landscape.mjs examples/cookly.landscape.json \
 *     --url http://localhost:8080 \
 *     --token "$ICC_TOKEN" \
 *     --org  "$ICC_ORG_ID" \
 *     --emit apps/cookly/src/lib/integrations.ts
 */

import { readFile, writeFile, mkdir } from 'node:fs/promises'
import { dirname } from 'node:path'

function arg(name, fallback) {
  const i = process.argv.indexOf(`--${name}`)
  return i > -1 ? process.argv[i + 1] : fallback
}

const file = process.argv[2]
const url = arg('url', process.env.ICC_URL ?? 'http://localhost:8080')
const token = arg('token', process.env.ICC_TOKEN)
const org = arg('org', process.env.ICC_ORG_ID)
const emit = arg('emit')

if (!file || !token || !org) {
  console.error('usage: apply-landscape.mjs <file.json> --token <jwt> --org <organization-id>')
  console.error('the token is a bearer token from the auth service; the org id comes from /api/v1/me')
  process.exit(1)
}

async function call(method, path, body) {
  const res = await fetch(`${url}/api/v1${path}`, {
    method,
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${token}`,
      'X-Organization-Id': org,
    },
    body: body === undefined ? undefined : JSON.stringify(body),
  })
  if (!res.ok) {
    const text = await res.text().catch(() => '')
    throw new Error(`${method} ${path} -> ${res.status} ${text.slice(0, 300)}`)
  }
  return res.status === 204 ? null : res.json()
}

const landscape = JSON.parse(await readFile(file, 'utf8'))

// ---- systems and components -------------------------------------------------

const existingSystems = await call('GET', '/systems')
const systemByName = new Map(existingSystems.map((s) => [s.name, s]))

for (const spec of landscape.systems) {
  let system = systemByName.get(spec.name)
  if (!system) {
    system = await call('POST', '/systems', {
      name: spec.name,
      system_type: spec.type,
      criticality: spec.criticality ?? 'MEDIUM',
      description: spec.description ?? null,
    })
    systemByName.set(spec.name, system)
    console.log(`+ system    ${spec.name}`)
  }

  const existing = await call('GET', `/components?system_id=${system.id}`)
  const byName = new Map(existing.map((c) => [c.name, c]))

  for (const component of spec.components ?? []) {
    if (byName.has(component.name)) continue
    const created = await call('POST', '/components', {
      system_id: system.id,
      name: component.name,
      component_type: component.type ?? 'SERVICE',
    })
    byName.set(component.name, created)
    console.log(`+ component ${spec.name}/${component.name}`)
  }

  spec._components = byName
}

// ---- integrations -----------------------------------------------------------

const types = await call('GET', '/integration-types')
const typeByKey = new Map(types.map((t) => [t.key, t]))

const existingIntegrations = await call('GET', '/integrations')
const integrationByName = new Map(existingIntegrations.map((i) => [i.name, i]))

/** Resolves "System Name/Component Name" to a component id. */
function resolve(reference) {
  const [systemName, componentName] = reference.split('/')
  const spec = landscape.systems.find((s) => s.name === systemName)
  const component = spec?._components?.get(componentName)
  if (!component) throw new Error(`unknown component: ${reference}`)
  return component.id
}

const emitted = {}

for (const spec of landscape.integrations) {
  let integration = integrationByName.get(spec.name)
  if (!integration) {
    const type = typeByKey.get(spec.type ?? 'HTTP')
    if (!type) throw new Error(`unknown integration type: ${spec.type}`)

    integration = await call('POST', '/integrations', {
      name: spec.name,
      source_component_id: resolve(spec.from),
      destination_component_id: resolve(spec.to),
      integration_type_id: type.id,
      criticality: spec.criticality ?? 'MEDIUM',
    })
    console.log(`+ integration ${spec.name}`)
  }
  // Slugs, not ids. A slug is the same in every instance, so the file this
  // writes is committed once rather than regenerated per environment.
  emitted[spec.constant ?? spec.name] = integration.slug
}

// ---- the ids the application needs ------------------------------------------

console.log('\nintegration slugs:')
for (const [name, id] of Object.entries(emitted)) {
  console.log(`  ${name.padEnd(30)} ${id}`)
}

if (emit) {
  const lines = [
    '// Written by scripts/apply-landscape.mjs, then safe to edit and commit.',
    '//',
    '// Slugs rather than ids: a slug is the same in every instance, so one',
    '// build reports to a laptop and to production without this file being',
    '// regenerated. Renaming an integration in the UI keeps its slug.',
    '',
    'export const INTEGRATIONS = {',
    ...Object.entries(emitted).map(([name, id]) => `  ${name}: '${id}',`),
    '} as const',
    '',
  ]
  await mkdir(dirname(emit), { recursive: true })
  await writeFile(emit, lines.join('\n'), 'utf8')
  console.log(`\nwrote ${emit}`)
}
