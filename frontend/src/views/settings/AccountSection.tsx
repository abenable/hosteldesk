import { useState } from 'react';
import { Card } from '../../components/ui/Card.tsx';
import { Button } from '../../components/ui/Button.tsx';
import { Input } from '../../components/ui/Input.tsx';
import { Badge } from '../../components/ui/Badge.tsx';
import { addToast } from '../../lib/toasts.ts';
import { login, updateStaffUser, type StaffUser } from '../../lib/tauri.ts';

type Props = {
  user: StaffUser;
};

export function AccountSection({ user }: Props) {
  const [currentPassword, setCurrentPassword] = useState('');
  const [newPassword, setNewPassword] = useState('');
  const [confirm, setConfirm] = useState('');
  const [busy, setBusy] = useState(false);

  const passwordsMatch = newPassword === confirm;
  const passwordLong = newPassword.length >= 8;
  const valid =
    currentPassword.length > 0 && passwordLong && passwordsMatch;

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!valid) return;
    setBusy(true);
    try {
      await login({ username: user.username, password: currentPassword });
      await updateStaffUser({
        user_id: user.id,
        name: user.name,
        username: user.username,
        role: user.role,
        password: newPassword,
      });
      addToast('Password changed.', 'success');
      setCurrentPassword('');
      setNewPassword('');
      setConfirm('');
    } catch {
      // login or update error surfaced as toast
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="flex flex-col gap-6">
      <Card shadow className="p-6">
        <h2 className="text-title-md text-ink">Your account</h2>
        <dl className="mt-4 flex flex-col gap-3 text-body-md">
          <div>
            <dt className="text-caption text-muted">Name</dt>
            <dd className="text-ink">{user.name}</dd>
          </div>
          <div>
            <dt className="text-caption text-muted">Email</dt>
            <dd className="text-ink">{user.username}</dd>
          </div>
          <div>
            <dt className="text-caption text-muted">Role</dt>
            <dd className="text-ink">
              <Badge className="capitalize">{user.role}</Badge>
            </dd>
          </div>
        </dl>
      </Card>

      <Card shadow className="p-6">
        <h2 className="text-title-md text-ink">Change password</h2>
        <p className="mt-1 text-body-sm text-muted">
          Enter your current password, then choose a new one.
        </p>
        <form onSubmit={handleSubmit} className="mt-4 flex flex-col gap-4">
          <Input
            label="Current password"
            type="password"
            value={currentPassword}
            onChange={(e) => setCurrentPassword(e.target.value)}
            required
          />
          <Input
            label="New password"
            type="password"
            value={newPassword}
            onChange={(e) => setNewPassword(e.target.value)}
            hint="At least 8 characters."
            error={
              newPassword.length > 0 && !passwordLong
                ? 'Password must be at least 8 characters.'
                : undefined
            }
          />
          <Input
            label="Confirm new password"
            type="password"
            value={confirm}
            onChange={(e) => setConfirm(e.target.value)}
            error={
              confirm.length > 0 && !passwordsMatch
                ? 'Passwords do not match.'
                : undefined
            }
          />
          <div className="flex justify-end">
            <Button type="submit" disabled={busy || !valid}>
              {busy ? 'Updating…' : 'Change password'}
            </Button>
          </div>
        </form>
      </Card>
    </div>
  );
}
