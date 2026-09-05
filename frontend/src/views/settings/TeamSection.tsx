import { useEffect, useState } from 'react';
import { Card } from '../../components/ui/Card.tsx';
import { Button } from '../../components/ui/Button.tsx';
import { Input } from '../../components/ui/Input.tsx';
import { Select } from '../../components/ui/Select.tsx';
import { Modal } from '../../components/ui/Modal.tsx';
import { Table, Th, Td, Tr } from '../../components/ui/Table.tsx';
import { EmptyState } from '../../components/ui/EmptyState.tsx';
import { addToast } from '../../lib/toasts.ts';
import { listUsers, createUser, updateUser, deleteUser, type User } from '../../lib/tauri.ts';

type Props = {
  hostelId: number;
};

export function TeamSection({ hostelId }: Props) {
  const [staff, setStaff] = useState<User[]>([]);
  const [loading, setLoading] = useState(true);
  const [addOpen, setAddOpen] = useState(false);
  const [editingManager, setEditingManager] = useState<User | null>(null);
  const [refreshKey, setRefreshKey] = useState(0);

  useEffect(() => {
    if (!hostelId) return;
    let cancelled = false;
    setLoading(true);
    listUsers({ hostel_id: hostelId })
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
        <Button size="sm" onClick={() => setAddOpen(true)}>
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
              <Button size="sm" onClick={() => setAddOpen(true)}>
                Add staff
              </Button>
            }
          />
        </div>
      ) : (
        <div className="mt-4 flex flex-col gap-6">
          <StaffTable title="Administrators" staff={staff.filter((s) => s.role === 'admin')} />
          <StaffTable title="Managers" staff={staff.filter((s) => s.role === 'manager')} onEdit={setEditingManager} />
        </div>
      )}

      {addOpen && (
        <StaffFormModal
          open
          onOpenChange={() => setAddOpen(false)}
          hostelId={hostelId}
          onDone={() => {
            setAddOpen(false);
            refresh();
          }}
        />
      )}

      {editingManager && (
        <EditManagerModal
          manager={editingManager}
          onOpenChange={() => setEditingManager(null)}
          onUpdated={() => {
            setEditingManager(null);
            refresh();
          }}
          onDeleted={() => {
            setEditingManager(null);
            refresh();
          }}
        />
      )}
    </Card>
  );
}

function StaffTable({
  title,
  staff,
  onEdit,
}: {
  title: string;
  staff: User[];
  onEdit?: (user: User) => void;
}) {
  return (
    <div>
      <h3 className="text-title-sm text-ink">{title}</h3>
      {staff.length === 0 ? (
        <p className="mt-2 text-body-sm text-muted">None yet.</p>
      ) : (
        <Table className="mt-2">
          <thead>
            <tr>
              <Th>Name</Th>
              <Th>Email</Th>
              <Th>Created</Th>
              {onEdit && <Th className="text-right">Actions</Th>}
            </tr>
          </thead>
          <tbody>
            {staff.map((s) => (
              <Tr key={s.id}>
                <Td>{s.full_name}</Td>
                <Td>{s.email}</Td>
                <Td>{new Date(s.created_at).toLocaleDateString()}</Td>
                {onEdit && (
                  <Td className="text-right">
                    <Button variant="tertiary" size="sm" onClick={() => onEdit(s)}>
                      Edit
                    </Button>
                  </Td>
                )}
              </Tr>
            ))}
          </tbody>
        </Table>
      )}
    </div>
  );
}

function EditManagerModal({
  manager,
  onOpenChange,
  onUpdated,
  onDeleted,
}: {
  manager: User;
  onOpenChange: (open: boolean) => void;
  onUpdated: () => void;
  onDeleted: () => void;
}) {
  const [name, setName] = useState(manager.full_name);
  const [email, setEmail] = useState(manager.email);
  const [password, setPassword] = useState('');
  const [busy, setBusy] = useState(false);
  const [confirmingDelete, setConfirmingDelete] = useState(false);
  const [deleting, setDeleting] = useState(false);

  const passwordLong = password.length === 0 || password.length >= 8;
  const valid = name.trim() && email.trim() && passwordLong;

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!valid) return;
    setBusy(true);
    try {
      await updateUser({
        user_id: manager.id,
        full_name: name.trim(),
        email: email.trim(),
        role: manager.role,
        password: password.trim() || undefined,
      });
      addToast('Manager updated.', 'success');
      onUpdated();
    } catch {
      // toast surfaced
    } finally {
      setBusy(false);
    }
  }

  async function handleDelete() {
    if (!confirmingDelete) {
      setConfirmingDelete(true);
      return;
    }
    setDeleting(true);
    try {
      await deleteUser({ user_id: manager.id });
      addToast('Manager account deleted.', 'success');
      onDeleted();
    } catch {
      // toast surfaced
    } finally {
      setDeleting(false);
    }
  }

  return (
    <Modal
      open
      onOpenChange={onOpenChange}
      title="Edit manager"
      description="Update this manager's details, or remove their account."
    >
      <form onSubmit={handleSubmit} className="flex flex-col gap-4">
        <Input label="Name" value={name} onChange={(e) => setName(e.target.value)} required />
        <Input label="Email" type="email" value={email} onChange={(e) => setEmail(e.target.value)} required />
        <Input
          label="New password (optional)"
          type="password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          hint="Leave blank to keep the current password. At least 8 characters."
          error={!passwordLong ? 'Password must be at least 8 characters.' : undefined}
        />
        <div className="flex items-center justify-between gap-3">
          <Button type="button" variant="secondary" onClick={handleDelete} disabled={deleting}>
            {deleting ? 'Deleting…' : confirmingDelete ? 'Confirm delete' : 'Delete manager'}
          </Button>
          <Button type="submit" disabled={busy || !valid}>
            {busy ? 'Saving…' : 'Save changes'}
          </Button>
        </div>
      </form>
    </Modal>
  );
}

function StaffFormModal({
  open,
  onOpenChange,
  hostelId,
  onDone,
}: {
  open: boolean;
  onOpenChange: (o: boolean) => void;
  hostelId: number;
  onDone: () => void;
}) {
  const [name, setName] = useState('');
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [role, setRole] = useState('manager');
  const [busy, setBusy] = useState(false);

  const passwordLong = password.length >= 8;
  const valid = name.trim() && email.trim() && passwordLong;

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!valid || !hostelId) return;
    setBusy(true);
    try {
      await createUser({
        hostel_id: hostelId,
        full_name: name.trim(),
        email: email.trim(),
        password,
        role,
      });
      addToast('Staff account created.', 'success');
      setName('');
      setEmail('');
      setPassword('');
      onDone();
    } catch {
      // toast surfaced
    } finally {
      setBusy(false);
    }
  }

  return (
    <Modal open={open} onOpenChange={onOpenChange} title="Add staff member" description="Create a new account.">
      <form onSubmit={handleSubmit} className="flex flex-col gap-4">
        <Select label="Role" value={role} onChange={(e) => setRole(e.target.value)}>
          <option value="admin">Admin</option>
          <option value="manager">Manager</option>
        </Select>
        <Input label="Name" value={name} onChange={(e) => setName(e.target.value)} required />
        <Input label="Email" type="email" value={email} onChange={(e) => setEmail(e.target.value)} required />
        <Input
          label="Password"
          type="password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          hint="At least 8 characters."
          error={password.length > 0 && !passwordLong ? 'Password must be at least 8 characters.' : undefined}
        />
        <div className="flex justify-end">
          <Button type="submit" disabled={busy || !valid}>
            Add
          </Button>
        </div>
      </form>
    </Modal>
  );
}
