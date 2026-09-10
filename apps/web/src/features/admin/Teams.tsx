import { useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { api, ApiError } from '../../lib/api'
import type { Member, Team, TeamMember } from '../../lib/types'
import { Badge, Card, EmptyState, PageHeader } from '../../components/ui'
import { buttonClass, inputClass } from '../../lib/format'

export function Teams() {
  const [name, setName] = useState('')
  const queryClient = useQueryClient()

  const teams = useQuery({ queryKey: ['teams'], queryFn: () => api.get<Team[]>('/teams') })

  const create = useMutation({
    mutationFn: () => api.post<Team>('/teams', { name }),
    onSuccess: () => {
      setName('')
      void queryClient.invalidateQueries({ queryKey: ['teams'] })
    },
  })

  return (
    <div>
      <PageHeader
        title="Teams"
        subtitle="Who is responsible for what. Separate from organization membership, which is what you are allowed to do."
      />

      <Card className="mb-6">
        <form
          className="flex gap-2"
          onSubmit={(e) => {
            e.preventDefault()
            if (name.trim()) create.mutate()
          }}
        >
          <input
            className={inputClass}
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="New team name"
          />
          <button className={buttonClass} disabled={create.isPending || !name.trim()}>
            Create
          </button>
        </form>
        {create.error && (
          <p className="mt-2 text-sm text-red-400">
            {create.error instanceof ApiError ? create.error.message : 'Could not create it.'}
          </p>
        )}
      </Card>

      {teams.isPending && <p className="text-sm text-neutral-500">Loading…</p>}
      {teams.data?.length === 0 && (
        <EmptyState
          title="No teams yet."
          hint="Create one, then set it as the owner of the systems it looks after."
        />
      )}

      <div className="space-y-3">
        {teams.data?.map((team) => <TeamRow key={team.id} team={team} />)}
      </div>
    </div>
  )
}

function TeamRow({ team }: { team: Team }) {
  const [open, setOpen] = useState(false)
  const queryClient = useQueryClient()

  const remove = useMutation({
    mutationFn: () => api.delete(`/teams/${team.id}`),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ['teams'] }),
  })

  return (
    <Card>
      <div className="flex items-start gap-3">
        <button className="flex flex-1 items-start gap-3 text-left" onClick={() => setOpen(!open)}>
          <span className="mt-1 text-neutral-600">{open ? '▾' : '▸'}</span>
          <div className="min-w-0">
            <div className="flex flex-wrap items-center gap-2">
              <span className="text-sm font-medium text-neutral-100">{team.name}</span>
              <Badge className="border-neutral-700 bg-neutral-800 text-neutral-400">
                {team.member_count} member{team.member_count === 1 ? '' : 's'}
              </Badge>
              {team.owned_systems > 0 && (
                <Badge className="border-neutral-700 bg-neutral-800 text-neutral-500">
                  {team.owned_systems} systems
                </Badge>
              )}
              {team.owned_integrations > 0 && (
                <Badge className="border-neutral-700 bg-neutral-800 text-neutral-500">
                  {team.owned_integrations} integrations
                </Badge>
              )}
            </div>
            {team.description && (
              <p className="mt-1 text-sm text-neutral-500">{team.description}</p>
            )}
          </div>
        </button>
        <button
          className="text-xs text-neutral-600 hover:text-red-400"
          onClick={() => remove.mutate()}
          title="What this team owns stays; it just becomes unowned."
        >
          Delete
        </button>
      </div>

      {open && (
        <div className="mt-4 border-t border-neutral-800 pt-4 pl-6">
          <TeamMembers teamId={team.id} />
        </div>
      )}
    </Card>
  )
}

function TeamMembers({ teamId }: { teamId: string }) {
  const [userId, setUserId] = useState('')
  const queryClient = useQueryClient()

  const members = useQuery({
    queryKey: ['team-members', teamId],
    queryFn: () => api.get<TeamMember[]>(`/teams/${teamId}/members`),
  })
  // Only people already in the organization can join a team.
  const candidates = useQuery({
    queryKey: ['organization-members'],
    queryFn: () => api.get<Member[]>('/organization/members'),
  })

  const add = useMutation({
    mutationFn: () => api.post(`/teams/${teamId}/members`, { user_id: userId }),
    onSuccess: () => {
      setUserId('')
      void queryClient.invalidateQueries({ queryKey: ['team-members', teamId] })
      void queryClient.invalidateQueries({ queryKey: ['teams'] })
    },
  })
  const remove = useMutation({
    mutationFn: (id: string) => api.delete(`/teams/${teamId}/members/${id}`),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ['team-members', teamId] })
      void queryClient.invalidateQueries({ queryKey: ['teams'] })
    },
  })

  const inTeam = new Set(members.data?.map((m) => m.user_id))
  const available = candidates.data?.filter((c) => !inTeam.has(c.user_id)) ?? []

  return (
    <div>
      <h3 className="mb-2 text-xs font-medium uppercase tracking-wide text-neutral-500">Members</h3>

      {members.data?.length === 0 && <p className="mb-2 text-sm text-neutral-600">Nobody yet.</p>}

      <div className="mb-3 space-y-1">
        {members.data?.map((m) => (
          <div key={m.user_id} className="flex items-center gap-2 text-sm text-neutral-300">
            <span>{m.display_name}</span>
            <span className="text-xs text-neutral-600">{m.email}</span>
            <button
              className="ml-auto text-xs text-neutral-600 hover:text-red-400"
              onClick={() => remove.mutate(m.user_id)}
            >
              Remove
            </button>
          </div>
        ))}
      </div>

      <form
        className="flex gap-2"
        onSubmit={(e) => {
          e.preventDefault()
          if (userId) add.mutate()
        }}
      >
        <select
          className={`${inputClass} flex-1`}
          value={userId}
          onChange={(e) => setUserId(e.target.value)}
        >
          <option value="">Add someone from the organization…</option>
          {available.map((c) => (
            <option key={c.user_id} value={c.user_id}>
              {c.display_name} ({c.email})
            </option>
          ))}
        </select>
        <button className={buttonClass} disabled={add.isPending || !userId}>Add</button>
      </form>

      {add.error && (
        <p className="mt-2 text-sm text-red-400">
          {add.error instanceof ApiError ? add.error.message : 'Could not add them.'}
        </p>
      )}
    </div>
  )
}
