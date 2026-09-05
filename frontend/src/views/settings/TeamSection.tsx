import { useEffect, useState } from 'react';
import { Card } from '../../components/ui/Card.tsx';
import { Button } from '../../components/ui/Button.tsx';
import { Input } from '../../components/ui/Input.tsx';
import { Select } from '../../components/ui/Select.tsx';
import { Badge } from '../../components/ui/Badge.tsx';
import { Modal } from '../../components/ui/Modal.tsx';
import { Table, Th, Td, Tr } from '../../components/ui/Table.tsx';
import { EmptyState } from '../../components/ui/EmptyState.tsx';
import { addToast } from '../../lib/toasts.ts';
import {
  listStaffUsers,
  createStaffUser,
  updateStaffUser,
  deleteStaffUser,
  type StaffUser,
} from '../../lib/tauri.ts';

type Props = {
  hostelId: number;
};

type ModalMode =
  | { kind: 'closed' }
  | { kind: 'add' }
  | { kind: 'edit'; user: StaffUser }
  | { kind: 'reset'; user: StaffUser }
  | { kind: 'delete'; user: StaffUser };

export function TeamSection({ hostelId }: Props) {
  const [staff, setStaff] = useState<StaffUser[]>([]);
  const [loading, setLoading] = useState(true);
  const [modal, setModal] = useState<ModalMode>({ kind: 'closed' });
  const [refreshKey, setRefreshKey] = useState(0);

  useEffect(() => {
    if (!hostelId) return;
    let cancelled = false;
    setLoading(true);
    listStaffUsers({ hostel_id: hostelId })
      .then((s) => {
        if (!cancelled) setStaff(s);
      })
      .catch(() => {})
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [hostelId, refreshKey]);

  function refresh() {
    setRefreshKey((k) => k + 1);
  }

  return (
    <Card shadow className="p-6">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-title-md text-ink">Staff accounts</h2>
          <p className="mt-1 text-body-sm text-muted">
            Manage who can sign in and use HostelDesk.
          </p>
        </div>
        <Button size="sm" onClick={() => setModal({ kind: 'add' })}>
          Add staff
        </Button>
      </div>

      {loading ? (
        <div className="mt-4 flex flex-col gap-3">
          {[...Array(3)].map((_, i) => (
            <div key={i} className="h-10 animate-pulse rounded-md bg-surface-soft" />
          ))}
        </div>
      ) : staff.length === 0 ? (
        <div className="mt-4">
          <EmptyState
            title="No staff accounts yet"
            description="Add a staff member to let them sign in."
            action={
              <Button size="sm" onClick={() => setModal({ kind: 'add' })}>
                Add staff
              </Button>
            }
          />
        </div>
      ) : (
        <>
          <Table className="mt-4">
            <thead>
              <tr>
                <Th>Name</Th>
                <Th>Email</Th>
                <Th>Role</Th>
                <Th>Created</Th>
                <Th className="text-right">Actions</Th>
              </tr>
            </thead>
            <tbody>
              {staff.map((s) => (
                <Tr key={s.id}>
                  <Td>{s.name}</Td>
                  <Td>{s.username}</Td>
                  <Td>
                    <Badge className="capitalize">{s.role}</Badge>
                  </Td>
                  <Td>{new Date(s.created_at).toLocaleDateString()}</Td>
                  <Td>
                    <div className="flex justify-end gap-2">
                      <Button
                        variant="tertiary"
                        size="sm"
                        onClick={() => setModal({ kind: 'edit', user: s })}
                      >
                        Edit
                      </Button>
                      <Button
                        variant="tertiary"
                        size="sm"
                        onClick={() => setModal({ kind: 'reset', user: s })}
                      >
                        Reset password
                      </Button>
                      <Button
                        variant="tertiary"
                        size="sm"
                        onClick={() => setModal({ kind: 'delete', user: s })}
                      >
                        Delete
                      </Button>
                    </div>
                  </Td>
                </Tr>
              ))}
            </tbody>
          </Table>
          <p className="mt-3 text-caption-sm text-muted">
            To change a password, use the Reset password action.
          </p>
        </>
      )}

      {modal.kind === 'add' && (
        <StaffFormModal
          open
          onOpenChange={() => setModal({ kind: 'closed' })}
          hostelId={hostelId}
          title="Add staff member"
          description="Create a new account."
          onDone={() => {
            setModal({ kind: 'closed' });
            refresh();
          }}
        />
      )}

      {modal.kind === 'edit' && (
        <StaffFormModal
          open
          onOpenChange={() => setModal({ kind: 'closed' })}
          hostelId={hostelId}
          user={modal.user}
          title="Edit staff member"
          description="Update name, email or role."
          onDone={() => {
            setModal({ kind: 'closed' });
            refresh();
          }}
        />
      )}

      {modal.kind === 'reset' && (
        <ResetPasswordModal
          open
          onOpenChange={() => setModal({ kind: 'closed' })}
          user={modal.user}
          onDone={() => setModal({ kind: 'closed' })}
        />
      )}

      {modal.kind === 'delete' && (
        <DeleteStaffModal
          open
          onOpenChange={() => setModal({ kind: 'closed' })}
          user={modal.user}
          onDone={() => {
            setModal({ kind: 'closed' });
            refresh();
          }}
        />
      )}
    </Card>
  );
}

function StaffFormModal({
  open,
  onOpenChange,
  hostelId,
  user,
  title,
  description,
  onDone,
}: {
  open: boolean;
  onOpenChange: (o: boolean) => void;
  hostelId: number;
  user?: StaffUser;
  title: string;
  description: string;
  onDone: () => void;
}) {
  const [name, setName] = useState(user?.name ?? '');
  const [username, setUsername] = useState(user?.username ?? '');
  const [password, setPassword] = useState('');
  const [confirm, setConfirm] = useState('');
  const [role, setRole] = useState(user?.role ?? 'manager');
  const [busy, setBusy] = useState(false);

  const passwordsMatch = password === confirm;
  const passwordLong = password.length >= 8;
  const passwordRequired = !user;
  const passwordValid = passwordRequired ? passwordLong && passwordsMatch : !password || (passwordLong && passwordsMatch);
  const valid = name.trim() && username.trim() && passwordValid;

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!valid || !hostelId) return;
    setBusy(true);
    try {
      if (user) {
        await updateStaffUser({
          user_id: user.id,
          name: name.trim(),
          username: username.trim(),
          role,
          password: password.trim() || undefined,
        });
        addToast('Staff member updated.', 'success');
      } else {
        await createStaffUser({
          hostel_id: hostelId,
          name: name.trim(),
          username: username.trim(),
          password,
          role,
        });
        addToast('Staff account created.', 'success');
      }
      setName('');
      setUsername('');
      setPassword('');
      setConfirm('');
      onDone();
    } catch {
      // toast surfaced
    } finally {
      setBusy(false);
    }
  }

  return (
    <Modal open={open} onOpenChange={onOpenChange} title={title} description={description}>
      <form onSubmit={handleSubmit} className="flex flex-col gap-4">
        <Input label="Name" value={name} onChange={(e) => setName(e.target.value)} required />
        <Input label="Email" type="email" value={username} onChange={(e) => setUsername(e.target.value)} required />
        <Input
          label={user ? 'New password (optional)' : 'Password'}
          type="password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          hint={user ? undefined : 'At least 8 characters.'}
          error={password.length > 0 && !passwordLong ? 'Password must be at least 8 characters.' : undefined}
        />
        {user ? (
          <Input
            label="Confirm new password"
            type="password"
            value={confirm}
            onChange={(e) => setConfirm(e.target.value)}
            error={confirm.length > 0 && !passwordsMatch ? 'Passwords do not match.' : undefined}
          />
        ) : (
          <Input
            label="Confirm password"
            type="password"
            value={confirm}
            onChange={(e) => setConfirm(e.target.value)}
            error={confirm.length > 0 && !passwordsMatch ? 'Passwords do not match.' : undefined}
          />
        )}
        <Select label="Role" value={role} onChange={(e) => setRole(e.target.value)}>
          <option value="admin">Admin</option>
          <option value="manager">Manager</option>
        </Select>
        <div className="flex justify-end gap-3">
          <Button variant="tertiary" onClick={() => onOpenChange(false)} disabled={busy}>
            Cancel
          </Button>
          <Button type="submit" disabled={busy || !valid}>
            {user ? 'Save changes' : 'Add'}
          </Button>
        </div>
      </form>
    </Modal>
  );
}

function ResetPasswordModal({
  open,
  onOpenChange,
  user,
  onDone,
}: {
  open: boolean;
  onOpenChange: (o: boolean) => void;
  user: StaffUser;
  onDone: () => void;
}) {
  const [password, setPassword] = useState('');
  const [confirm, setConfirm] = useState('');
  const [busy, setBusy] = useState(false);

  const passwordsMatch = password === confirm;
  const passwordLong = password.length >= 8;
  const valid = passwordLong && passwordsMatch;

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!valid) return;
    setBusy(true);
    try {
      await updateStaffUser({
        user_id: user.id,
        name: user.name,
        username: user.username,
        role: user.role,
        password,
      });
      addToast('Password updated.', 'success');
      setPassword('');
      setConfirm('');
      onDone();
    } catch {
      // toast surfaced
    } finally {
      setBusy(false);
    }
  }

  return (
    <Modal
      open={open}
      onOpenChange={onOpenChange}
      title="Reset password"
      description={`Set a new password for ${user.name}.`}
    >
      <form onSubmit={handleSubmit} className="flex flex-col gap-4">
        <Input
          label="New password"
          type="password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          hint="At least 8 characters."
          error={password.length > 0 && !passwordLong ? 'Password must be at least 8 characters.' : undefined}
        />
        <Input
          label="Confirm password"
          type="password"
          value={confirm}
          onChange={(e) => setConfirm(e.target.value)}
          error={confirm.length > 0 && !passwordsMatch ? 'Passwords do not match.' : undefined}
        />
        <div className="flex justify-end gap-3">
          <Button variant="tertiary" onClick={() => onOpenChange(false)} disabled={busy}>
            Cancel
          </Button>
          <Button type="submit" disabled={busy || !valid}>
            Reset password
          </Button>
        </div>
      </form>
    </Modal>
  );
}

function DeleteStaffModal({
  open,
  onOpenChange,
  user,
  onDone,
}: {
  open: boolean;
  onOpenChange: (o: boolean) => void;
  user: StaffUser;
  onDone: () => void;
}) {
  const [busy, setBusy] = useState(false);

  async function handleDelete() {
    setBusy(true);
    try {
      await deleteStaffUser({ user_id: user.id });
      addToast('Staff account deleted.', 'success');
      onDone();
    } catch {
      // toast surfaced
    } finally {
      setBusy(false);
    }
  }

  return (
    <Modal
      open={open}
      onOpenChange={onOpenChange}
      title="Delete staff account?"
      description={`This will permanently remove ${user.name}'s account. This action cannot be undone.`}
    >
      <div className="flex justify-end gap-3">
        <Button variant="tertiary" onClick={() => onOpenChange(false)} disabled={busy}>
          Cancel
        </Button>
        <Button variant="secondary" onClick={handleDelete} disabled={busy}>
          {busy ? 'Deleting…' : 'Delete'}
        </Button>
      </div>
    </Modal>
  );
}
