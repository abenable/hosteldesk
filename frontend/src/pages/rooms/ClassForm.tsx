import { useState, type FormEvent } from 'react';
import { Alert, Button, Field, Input, Select } from '../../components/ui';
import * as api from '../../lib/api';
import { cleanMoney, isMoneyInput } from '../../lib/money';
import type { RoomClass } from '../../lib/types';
import { useAction } from '../../lib/useLoad';
import { ROOM_TYPES } from './room';

type Key = 'floor' | 'kind' | 'semester' | 'recess';

// What Rust's `parse::<i32>()` accepts (9 digits keeps it inside i32).
const WHOLE = /^[+-]?\d{1,9}$/;

const EMPTY: Record<Key, string> = { floor: '', kind: 'single', semester: '', recess: '' };

/** Field errors for a room class. Empty when it can be sent. */
function validate(v: Record<Key, string>): Partial<Record<Key, string>> {
  const errors: Partial<Record<Key, string>> = {};
  if (v.floor.trim() && !WHOLE.test(v.floor.trim()))
    errors.floor = 'Floor must be a whole number, or left blank.';
  if (!isMoneyInput(v.semester)) errors.semester = 'Enter a price above zero, like 450000.';
  if (!isMoneyInput(v.recess)) errors.recess = 'Enter a price above zero, like 200000.';
  return errors;
}

/** Props of {@link ClassForm}. */
interface ClassFormProps {
  hostelId: number;
  currency: string;
  /** The class to edit; omit to create one (the form empties after each save). */
  editing?: RoomClass;
  submitLabel: string;
  onSaved: (id: number) => void;
  onCancel?: () => void;
}

/**
 * Floor, type and the two prices of a room class (`save_room_class`). Its
 * rooms take their type, floor and prices from it. Admin only.
 */
export function ClassForm({ hostelId, currency, editing, submitLabel, onSaved, onCancel }: ClassFormProps) {
  const [values, setValues] = useState<Record<Key, string>>(() =>
    editing
      ? {
          floor: editing.floor === null ? '' : String(editing.floor),
          kind: editing.room_type,
          semester: editing.semester_rate,
          recess: editing.recess_rate,
        }
      : EMPTY,
  );
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
      const id = await api.saveRoomClass({
        hostel_id: hostelId,
        class_id: editing?.id ?? null,
        floor: floor ? Number(floor) : null,
        room_type: values.kind,
        semester_rate: cleanMoney(values.semester),
        recess_rate: cleanMoney(values.recess),
      });
      if (!editing) setValues(EMPTY);
      onSaved(id);
    });
  };

  const price = `${currency} per resident.`;
  return (
    <form onSubmit={submit} noValidate className="space-y-4">
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
        <Field label="Type">
          <Select value={values.kind} onChange={set('kind')}>
            {ROOM_TYPES.map((t) => (
              <option key={t.value} value={t.value}>
                {t.label}
              </option>
            ))}
          </Select>
        </Field>
        <Field label="Floor" hint="Optional." error={errors.floor}>
          <Input inputMode="numeric" value={values.floor} onChange={set('floor')} placeholder="e.g. 1" />
        </Field>
        <Field label="Semester price" hint={price} error={errors.semester}>
          <Input inputMode="decimal" value={values.semester} onChange={set('semester')} placeholder="e.g. 450000" />
        </Field>
        <Field label="Recess price" hint={price} error={errors.recess}>
          <Input inputMode="decimal" value={values.recess} onChange={set('recess')} placeholder="e.g. 200000" />
        </Field>
      </div>
      {save.error && <Alert title="Could not save the class">{save.error}</Alert>}
      <div className="flex justify-end gap-3">
        {onCancel && (
          <Button variant="secondary" onClick={onCancel} disabled={save.busy}>
            Cancel
          </Button>
        )}
        <Button type="submit" icon={editing ? undefined : 'plus'} loading={save.busy}>
          {submitLabel}
        </Button>
      </div>
    </form>
  );
}
