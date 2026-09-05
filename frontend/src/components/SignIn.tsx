import { useState } from 'react';
import { login, type User } from '../lib/tauri.ts';
import { setCurrentUser } from '../lib/current-user.ts';
import { Button } from './ui/Button.tsx';
import { Card } from './ui/Card.tsx';
import { Input } from './ui/Input.tsx';
import logo from '../assets/logo.png';

type Props = {
  onSignedIn: (user: User) => void;
};

export function SignIn({ onSignedIn }: Props) {
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleSubmit(event: React.FormEvent) {
    event.preventDefault();
    setError(null);
    if (!email.trim() || !password) {
      setError('Enter your email and password.');
      return;
    }

    setBusy(true);
    try {
      const user = await login({
        email: email.trim(),
        password,
      });
      setCurrentUser(user);
      onSignedIn(user);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="flex h-screen items-center justify-center overflow-y-auto bg-canvas p-4">
      <Card shadow className="w-full max-w-md p-8 text-center">
        <img
          src={logo}
          alt="HostelDesk"
          className="mx-auto mb-6 h-20 w-20 rounded-lg"
        />
        <h1 className="text-display-md text-ink">Welcome back</h1>
        <p className="mt-2 text-body-sm text-muted">
          Sign in to manage your hostel.
        </p>

        <form onSubmit={handleSubmit} className="mt-8 flex flex-col gap-4 text-left">
          <Input
            label="Email"
            name="email"
            type="email"
            autoComplete="email"
            value={email}
            onChange={(event) => setEmail(event.target.value)}
            error={error ?? undefined}
          />
          <Input
            label="Password"
            name="password"
            type="password"
            autoComplete="current-password"
            value={password}
            onChange={(event) => setPassword(event.target.value)}
          />
          <Button type="submit" disabled={busy}>
            {busy ? 'Signing in…' : 'Sign in'}
          </Button>
        </form>
      </Card>
    </div>
  );
}
