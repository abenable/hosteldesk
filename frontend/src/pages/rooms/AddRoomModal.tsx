import { useState, type FormEvent } from 'react';
import { Alert, Button, Field, Input, Modal, Select } from '../../components/ui';
import * as api from '../../lib/api';
import { useDesk } from '../../lib/desk';
import { cleanMoney, isMoneyInput } from '../../lib/money';
import { useAction } from '../../lib/useLoad';
import { ROOM_TYPES } from './room';

type Key = 'number' | 'kind' | 'capacity' | 'floor' | 'semester' | 'recess';

// What Rust's `parse::<i32>()` accepts (9 digits keeps it inside i32).
const WHOLE = /^[+-]?\d{1,9}$/;

/** Field errors for the add-room form, with ui.rs's messages. Empty when it can be sent. */
function validate(v: Record<Key, string>): Partial<Record<Key, string>> {
  const errors: Partial<Record<Key, string>> = {};
  if (!v.number.trim()) errors.number = 'Enter a room number.';
  if (!WHOLE.test(v.capacity.trim())) errors.capacity = 'Beds must be a whole number.';
  else if (Number(v.capacity.trim()) < 1) errors.capacity = 'A room needs at least one bed.';
  if (v.floor.trim() && !WHOLE.test(v.floor.trim()))
    errors.floor = 'Floor must be a whole number, or left blank.';
  if (!isMoneyInput(v.semester))
    errors.semester = 'Semester rate must be an amount above zero, like 450000.';
  if (!isMoneyInput(v.recess))
    errors.recess = 'Recess rate must be an amount above zero, like 450000.';
  return errors;
}

/** Props of {@link AddRoomModal}. */
interface AddRoomModalProps {
  open: boolean;
  onClose: () => void;
  /** After a successful add, with the new room's id (the page selects it). */
  onAdded: (id: number) => void;
}

/**
 * "Add room" dialog (ui.rs `add_room`). Admin only. The form state lives
 * here, so give it a new `key` each time it opens to start empty (closing it
 * through `open={false}` lets the dialog hand focus back to the opener).
 */
export function AddRoomModal({ open, onClose, onAdded }: AddRoomModalProps) {
  const desk = useDesk();
  const [values, setValues] = useState<Record<Key, string>>({
    number: '',
    kind: 'single',
    capacity: '1',
    floor: '',
    semester: '',
    recess: '',
  });
  const [errors, setErrors] = useState<Partial<Record<Key, string>>>({});
  const save = useAction();
  const set = (key: Key) => (e: { target: { value: string } }) =>
    setValues((v) => ({ ...v, [key]: e.target.value }));

  const submit = (event: FormEvent) => {
    event.preventDefault();
    const found = validate(values);
    setErrors(found);
    if (Object.keys(found).length > 0) return;
    void save.run(async () => {
      const floor = values.floor.trim();
      const id = await api.addRoom({
        hostel_id: desk.hostelId,
        room_number: values.number.trim(),
        room_type: values.kind,
        capacity: Number(values.capacity.trim()),
        floor: floor ? Number(floor) : null,
        semester_rate: cleanMoney(values.semester),
        recess_rate: cleanMoney(values.recess),
      });
      desk.toast({ tone: 'success', title: 'Room added.' });
      desk.reload();
      onAdded(id);
    });
  };

  const rate = `${desk.currency}, per resident. For example 450000.`;
  return (
    <Modal
      open={open}
      onClose={onClose}
      title="Add room"
      description="New rooms start empty and ready for check-in."
      onSubmit={submit}
      busy={save.busy}
      footer={
        <>
          <Button variant="secondary" onClick={onClose} disabled={save.busy}>
            Cancel
          </Button>
          <Button type="submit" loading={save.busy}>
            Add room
          </Button>
        </>
      }
    >
      <div className="grid grid-cols-2 gap-4">
        <Field label="Room number" error={errors.number}>
          <Input value={values.number} onChange={set('number')} placeholder="e.g. A12" />
        </Field>
        <Field label="Type">
          <Select value={values.kind} onChange={set('kind')}>
            {ROOM_TYPES.map((t) => (
              <option key={t.value} value={t.value}>
                {t.label}
              </option>
            ))}
          </Select>
        </Field>
        <Field label="Beds" error={errors.capacity}>
          <Input inputMode="numeric" value={values.capacity} onChange={set('capacity')} />
        </Field>
        <Field label="Floor" hint="Optional." error={errors.floor}>
          <Input inputMode="numeric" value={values.floor} onChange={set('floor')} />
        </Field>
      </div>
      <Field label="Semester rate" hint={rate} error={errors.semester}>
        <Input inputMode="decimal" value={values.semester} onChange={set('semester')} />
      </Field>
      <Field label="Recess rate" hint={rate} error={errors.recess}>
        <Input inputMode="decimal" value={values.recess} onChange={set('recess')} />
      </Field>
      {save.error && <Alert title="Could not add the room">{save.error}</Alert>}
    </Modal>
  );
}
