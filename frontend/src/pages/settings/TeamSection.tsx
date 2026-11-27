import { useState, type FormEvent } from 'react';
import {
  Alert,
  Badge,
  Button,
  Card,
  EmptyState,
  Field,
  Input,
  Modal,
  NameCell,
  Select,
  Skeleton,
  Table,
} from '../../components/ui';
import * as api from '../../lib/api';
import { useDesk } from '../../lib/desk';
import { looksLikeEmail } from '../../lib/format';
import type { Role, User } from '../../lib/types';
import { useAction, useLoad } from '../../lib/useLoad';
import Section from './Section';

const roleLabel = (role: string) => (role === 'admin' ? 'Administrator' : 'Manager');

/** Rust's raw errors for the two refusals staff can hit here, in plain words. */
function friendly(message: string): string {
  if (/only admin/i.test(message)) return 'This is the only administrator, so it cannot be removed.';
  if (/UNIQUE.*email/i.test(message)) return 'Another staff account already uses this email.';
  return message;
}

/** Settings → Team (admin): the staff who can sign in at this desk; add and remove them. */
export default function TeamSection() {
  const desk = useDesk();
  const users = useLoad(() => api.listUsers({ hostel_id: desk.hostelId }), [desk.hostelId]);
  const [adding, setAdding] = useState(false);
  const [addKey, setAddKey] = useState(0);
  const staff = users.data ?? [];
  const admins = staff.filter((u) => u.role === 'admin').length;

  const openAdd = () => {
    setAddKey((k) => k + 1);
    setAdding(true);
  };

  const remove = (user: User) =>
    void desk.confirm({
      title: `Remove ${user.full_name}?`,
      message: `${user.email} will no longer be able to sign in.`,
      confirmLabel: 'Remove',
      tone: 'danger',
      action: async () => {
        try {
          await api.deleteUser({ user_id: user.id });
        } catch (error) {
          throw new Error(friendly(api.errorMessage(error)));
        }
        desk.toast({ tone: 'success', title: 'Staff member removed.' });
        desk.reload();
      },
    });

  const addButton = (
    <Button variant="secondary" icon="user-plus" onClick={openAdd}>
      Add staff
    </Button>
  );

  let body;
  if (users.loading) {
    body = (
      <Card>
        <Skeleton rows={4} />
      </Card>
    );
  } else if (users.error && !users.data) {
    body = (
      <Alert title="Could not load staff" onRetry={users.reload}>
        {users.error}
      </Alert>
    );
  } else if (staff.length === 0) {
    body = (
      <Card>
        <EmptyState
          icon="user-group"
          title="No staff yet"
          description="Add the people who work the front desk so each signs in with their own account."
          action={addButton}
        />
      </Card>
    );
  } else {
    body = (
      <Card flush>
        <Table
          caption="Staff"
          rows={staff}
          rowKey={(u) => u.id}
          columns={[
            { header: 'Name', cell: (u) => <NameCell name={u.full_name} detail={u.email} /> },
            {
              header: 'Role',
              cell: (u) => <Badge tone={u.role === 'admin' ? 'info' : 'neutral'}>{roleLabel(u.role)}</Badge>,
            },
            {
              header: <span className="sr-only">Actions</span>,
              align: 'end',
              cell: (u) =>
                u.id === desk.user.id ? (
                  <span className="text-sm text-gray-500">You</span>
                ) : u.role === 'admin' && admins <= 1 ? (
                  <span className="text-sm text-gray-500">Only administrator</span>
                ) : (
                  <Button size="sm" variant="ghost" icon="trash" onClick={() => remove(u)}>
                    Remove<span className="sr-only"> {u.full_name}</span>
                  </Button>
                ),
            },
          ]}
        />
      </Card>
    );
  }

  return (
    <Section
      title="Staff"
      description={
        <>
          <p>
            {users.data
              ? `${staff.length} ${staff.length === 1 ? 'person' : 'people'} · ${admins} ${admins === 1 ? 'administrator' : 'administrators'}`
              : 'Everyone who can sign in at this desk.'}
          </p>
          <p className="mt-2">
            Managers run the front desk. Administrators can also change settings, manage staff and back up.
          </p>
        </>
      }
      action={users.data && staff.length > 0 ? addButton : undefined}
    >
      {body}
      <AddStaffModal key={addKey} open={adding} onClose={() => setAdding(false)} />
    </Section>
  );
}

type Key = 'name' | 'email' | 'password';

function AddStaffModal({ open, onClose }: { open: boolean; onClose: () => void }) {
  const desk = useDesk();
  const [values, setValues] = useState<Record<Key, string>>({ name: '', email: '', password: '' });
  const [role, setRole] = useState<Role>('manager');
  const [errors, setErrors] = useState<Partial<Record<Key, string>>>({});
  const create = useAction();
  const set = (key: Key) => (e: { target: { value: string } }) =>
    setValues((v) => ({ ...v, [key]: e.target.value }));

  const submit = (event: FormEvent) => {
    event.preventDefault();
    const found: typeof errors = {};
    if (!values.name.trim()) found.name = 'Enter their full name.';
    if (!looksLikeEmail(values.email)) found.email = 'Enter a valid email address.';
    if ([...values.password].length < 8) found.password = 'Use at least 8 characters.';
    setErrors(found);
    if (Object.keys(found).length > 0) return;
    void create.run(async () => {
      try {
        await api.createUser({
          hostel_id: desk.hostelId,
          full_name: values.name.trim(),
          email: values.email.trim(),
          password: values.password,
          role,
        });
      } catch (error) {
        throw new Error(friendly(api.errorMessage(error)));
      }
      desk.toast({ tone: 'success', title: 'Staff account created.' });
      desk.reload();
      onClose();
    });
  };

  return (
    <Modal
      open={open}
      onClose={onClose}
      title="Add staff"
      description="They sign in at this desk with this email and password."
      onSubmit={submit}
      busy={create.busy}
      footer={
        <>
          <Button variant="secondary" onClick={onClose} disabled={create.busy}>
            Cancel
          </Button>
          <Button type="submit" loading={create.busy}>
            Create staff account
          </Button>
        </>
      }
    >
      <Field label="Full name" error={errors.name}>
        <Input autoComplete="off" value={values.name} onChange={set('name')} />
      </Field>
      <Field label="Email" error={errors.email}>
        <Input type="email" autoComplete="off" value={values.email} onChange={set('email')} />
      </Field>
      <Field label="Password" hint="At least 8 characters. Share it with them in person." error={errors.password}>
        <Input type="password" autoComplete="new-password" value={values.password} onChange={set('password')} />
      </Field>
      <Field
        label="Role"
        hint={
          role === 'admin'
            ? 'Can also change settings, manage staff and back up.'
            : 'Runs the front desk: rooms, residents and payments.'
        }
      >
        <Select value={role} onChange={(e) => setRole(e.target.value as Role)}>
          <option value="manager">Manager</option>
          <option value="admin">Administrator</option>
        </Select>
      </Field>
      {create.error && <Alert title="Could not add this person">{create.error}</Alert>}
    </Modal>
  );
}
