import { useState } from 'react';
import { Card } from '../../components/ui/Card.tsx';
import { Button } from '../../components/ui/Button.tsx';
import { Input } from '../../components/ui/Input.tsx';
import { Badge } from '../../components/ui/Badge.tsx';
import { Modal } from '../../components/ui/Modal.tsx';
import { BillingSection } from './BillingSection.tsx';
import { addToast } from '../../lib/toasts.ts';
import { login, updateUser, type User } from '../../lib/tauri.ts';

type Props = {
  user: User;
};

export function AccountSection({ user }: Props) {
  const [passwordOpen, setPasswordOpen] = useState(false);
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
      await login({ email: user.email, password: currentPassword });
      await updateUser({
        user_id: user.id,
        full_name: user.full_name,
        email: user.email,
        role: user.role,
        password: newPassword,
      });
      addToast('Password changed.', 'success');
      setCurrentPassword('');
      setNewPassword('');
      setConfirm('');
      setPasswordOpen(false);
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
            <dd className="text-ink">{user.full_name}</dd>
          </div>
          <div>
            <dt className="text-caption text-muted">Email</dt>
            <dd className="text-ink">{user.email}</dd>
          </div>
          <div>
            <dt className="text-caption text-muted">Role</dt>
            <dd className="text-ink">
              <Badge className="capitalize">{user.role}</Badge>
            </dd>
          </div>
        </dl>
      </Card>

      <Card shadow className="flex items-center justify-between p-6">
        <div>
          <h2 className="text-title-md text-ink">Change password</h2>
          <p className="mt-1 text-body-sm text-muted">
            Update the password used to sign in.
          </p>
        </div>
        <Button variant="tertiary" onClick={() => setPasswordOpen(true)}>
          Change password
        </Button>
      </Card>

      {user.role === 'admin' && <BillingSection />}

      <Modal
        open={passwordOpen}
        onOpenChange={setPasswordOpen}
        title="Change password"
        description="Enter your current password, then choose a new one."
      >
        <form onSubmit={handleSubmit} className="flex flex-col gap-4">
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
      </Modal>
    </div>
  );
}
