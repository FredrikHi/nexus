import { useState } from 'react'
import { signIn, signUp } from '../../lib/authClient'

type Mode = 'sign-in' | 'sign-up'

export function SignIn() {
  const [mode, setMode] = useState<Mode>('sign-in')
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const [name, setName] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  async function submit(e: React.FormEvent) {
    e.preventDefault()
    setError(null)
    setBusy(true)
    try {
      const result =
        mode === 'sign-in'
          ? await signIn.email({ email, password })
          : await signUp.email({ email, password, name: name || email })

      if (result.error) {
        setError(result.error.message ?? 'That did not work.')
      }
      // On success the session cookie is set and useSession re-renders the app.
    } catch {
      setError('Could not reach the sign-in service.')
    } finally {
      setBusy(false)
    }
  }

  return (
    <div className="flex min-h-full items-center justify-center bg-neutral-950 p-6">
      <div className="w-full max-w-sm">
        <div className="mb-8">
          <h1 className="text-lg font-semibold text-neutral-100">Integration Control Center</h1>
          <p className="mt-1 text-sm text-neutral-400">
            {mode === 'sign-in' ? 'Sign in to continue.' : 'Create an account to get started.'}
          </p>
        </div>

        <form onSubmit={submit} className="space-y-3">
          {mode === 'sign-up' && (
            <Field label="Name">
              <input
                className={inputClass}
                value={name}
                onChange={(e) => setName(e.target.value)}
                autoComplete="name"
                placeholder="Your name"
              />
            </Field>
          )}
          <Field label="Email">
            <input
              className={inputClass}
              type="email"
              required
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              autoComplete="email"
            />
          </Field>
          <Field label="Password">
            <input
              className={inputClass}
              type="password"
              required
              minLength={8}
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              autoComplete={mode === 'sign-in' ? 'current-password' : 'new-password'}
            />
          </Field>

          {error && (
            <p className="rounded border border-red-900 bg-red-950/50 px-3 py-2 text-sm text-red-300">
              {error}
            </p>
          )}

          <button
            type="submit"
            disabled={busy}
            className="w-full rounded bg-neutral-100 px-3 py-2 text-sm font-medium text-neutral-900 hover:bg-white disabled:opacity-50"
          >
            {busy ? 'Working…' : mode === 'sign-in' ? 'Sign in' : 'Create account'}
          </button>
        </form>

        <div className="my-4 flex items-center gap-3 text-xs text-neutral-600">
          <span className="h-px flex-1 bg-neutral-800" />
          or
          <span className="h-px flex-1 bg-neutral-800" />
        </div>

        <button
          onClick={() => signIn.social({ provider: 'google', callbackURL: '/' })}
          className="w-full rounded border border-neutral-700 px-3 py-2 text-sm text-neutral-200 hover:bg-neutral-900"
        >
          Continue with Google
        </button>
        <p className="mt-2 text-center text-xs text-neutral-600">
          Google is only available if the server has it configured.
        </p>

        <button
          onClick={() => {
            setMode(mode === 'sign-in' ? 'sign-up' : 'sign-in')
            setError(null)
          }}
          className="mt-6 w-full text-center text-sm text-neutral-400 hover:text-neutral-200"
        >
          {mode === 'sign-in' ? 'No account? Create one' : 'Already have an account? Sign in'}
        </button>
      </div>
    </div>
  )
}

const inputClass =
  'w-full rounded border border-neutral-700 bg-neutral-900 px-3 py-2 text-sm text-neutral-100 outline-none focus:border-neutral-500'

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className="block">
      <span className="mb-1 block text-xs text-neutral-400">{label}</span>
      {children}
    </label>
  )
}
