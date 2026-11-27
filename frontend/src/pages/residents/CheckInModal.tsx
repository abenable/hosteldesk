import { useState, type FormEvent } from 'react';
import { Alert, Button, Field, Input, Modal, Select } from '../../components/ui';
import * as api from '../../lib/api';
import { useDesk } from '../../lib/desk';
import { academicYear, blank, looksLikeEmail, TERMS } from '../../lib/format';
import { cleanMoney, isMoneyInput } from '../../lib/money';
import type { Term } from '../../lib/types';
import { useAction, useLoad } from '../../lib/useLoad';
import { freeRooms, rateForTerm, roomChoice } from './stay';

type Key = 'name' | 'phone' | 'email' | 'id_number' | 'kin' | 'kin_phone' | 'room' | 'rate' | 'year';

/** Props of {@link CheckInModal}. */
export interface CheckInModalProps {
  open: boolean;
  onClose: () => void;
  /** Check this past resident in again (no details to type); omit for a new resident. */
  returning?: { id: number; full_name: string } | null;
}

/**
 * Check a resident into a room (`check_in`; ui.rs
 * `check_in_form` / `check_in`). The rate is prefilled from the room for the
 * chosen term and stays editable. Render it keyed by an open counter so each
 * opening starts with an empty form.
 */
export default function CheckInModal({ open, onClose, returning }: CheckInModalProps) {
  const desk = useDesk();
  const rooms = useLoad(() => (open ? api.rooms(desk.hostelId) : Promise.resolve([])), [desk.hostelId, open]);
  const free = freeRooms(rooms.data ?? []);
  const [values, setValues] = useState<Record<Key, string>>(() => ({
    name: '',
    phone: '',
    email: '',
    id_number: '',
    kin: '',
    kin_phone: '',
    room: '',
    rate: '',
    year: academicYear(),
  }));
  const [term, setTerm] = useState<Term>('semester_1');
  const [errors, setErrors] = useState<Partial<Record<Key, string>>>({});
  const save = useAction();
  const set = (key: Key) => (e: { target: { value: string } }) => {
    setValues((v) => ({ ...v, [key]: e.target.value }));
    setErrors((errs) => ({ ...errs, [key]: undefined }));
  };

  // A new room or term prefills that room's rate for the term (ui.rs did the same).
  const pick = (roomId: string, nextTerm: Term) => {
    setTerm(nextTerm);
    const room = free.find((r) => String(r.id) === roomId);
    setValues((v) => ({ ...v, room: roomId, rate: room ? rateForTerm(room, nextTerm) : v.rate }));
    setErrors((errs) => ({ ...errs, room: undefined, rate: room ? undefined : errs.rate }));
  };

  const submit = (event: FormEvent) => {
    event.preventDefault();
    const room = free.find((r) => String(r.id) === values.room);
    const found: Partial<Record<Key, string>> = {};
    if (!returning && !values.name.trim()) found.name = "Enter the resident's full name.";
    if (!returning && values.email.trim() && !looksLikeEmail(values.email))
      found.email = 'Enter a valid email address.';
    if (!room) found.room = 'Choose a room.';
    if (!isMoneyInput(values.rate)) found.rate = 'Rate must be an amount above zero, like 450000.';
    if (!values.year.trim()) found.year = 'Enter the academic year, like 2026/2027.';
    setErrors(found);
    if (!room || Object.keys(found).length > 0) return;

    const name = returning ? returning.full_name : values.name.trim();
    void save.run(async () => {
      await api.checkIn({
        hostel_id: desk.hostelId,
        room_id: room.id,
        resident_id: returning?.id ?? null,
        full_name: returning ? null : name,
        phone: returning ? null : blank(values.phone),
        email: returning ? null : blank(values.email),
        id_number: returning ? null : blank(values.id_number),
        next_of_kin: returning ? null : blank(values.kin),
        next_of_kin_phone: returning ? null : blank(values.kin_phone),
        photo_path: null,
        rate_agreed: cleanMoney(values.rate),
        term,
        academic_year: values.year.trim(),
      });
      desk.toast({ tone: 'success', title: 'Checked in.', message: `${name} · Room ${room.room_number}` });
      desk.reload();
      onClose();
    });
  };

  const legend = 'text-xs font-medium tracking-wide text-gray-500 uppercase';

  return (
    <Modal
      open={open}
      onClose={onClose}
      size="lg"
      title={returning ? `Check in ${returning.full_name}` : 'Check in'}
      description={
        returning ? 'A new stay for a returning resident.' : 'A new resident and their first stay.'
      }
      onSubmit={submit}
      busy={save.busy}
      footer={
        <>
          <Button variant="secondary" onClick={onClose} disabled={save.busy}>
            Cancel
          </Button>
          <Button type="submit" loading={save.busy}>
            Check in
          </Button>
        </>
      }
    >
      {!returning && (
        <fieldset className="space-y-4">
          <legend className={legend}>Resident</legend>
          <div className="grid grid-cols-2 gap-4">
            <Field label="Full name" error={errors.name}>
              <Input autoComplete="off" value={values.name} onChange={set('name')} />
            </Field>
            <Field label="ID number">
              <Input autoComplete="off" value={values.id_number} onChange={set('id_number')} />
            </Field>
            <Field label="Phone">
              <Input type="tel" autoComplete="off" value={values.phone} onChange={set('phone')} />
            </Field>
            <Field label="Email" error={errors.email}>
              <Input type="email" autoComplete="off" value={values.email} onChange={set('email')} />
            </Field>
            <Field label="Next of kin">
              <Input autoComplete="off" value={values.kin} onChange={set('kin')} />
            </Field>
            <Field label="Next-of-kin phone">
              <Input type="tel" autoComplete="off" value={values.kin_phone} onChange={set('kin_phone')} />
            </Field>
          </div>
        </fieldset>
      )}

      <fieldset className="space-y-4">
        <legend className={legend}>Stay</legend>
        {rooms.error && !rooms.data ? (
          <Alert title="Could not load rooms" onRetry={rooms.reload}>
            {rooms.error}
          </Alert>
        ) : (
          <Field label="Room" error={errors.room}>
            <Select
              value={values.room}
              disabled={rooms.loading}
              onChange={(e) => pick(e.target.value, term)}
            >
              <option value="">{rooms.loading ? 'Loading rooms…' : 'Choose a room'}</option>
              {free.map((room) => (
                <option key={room.id} value={room.id}>
                  {roomChoice(room)}
                </option>
              ))}
            </Select>
          </Field>
        )}
        {!rooms.loading && rooms.data && free.length === 0 && (
          <Alert tone="warning" title="No room is taking check-ins.">
            {rooms.data.length === 0
              ? 'An administrator sets up the rooms first.'
              : 'Every room is under maintenance.'}
          </Alert>
        )}
        <div className="grid grid-cols-3 gap-4">
          <Field label="Term">
            <Select value={term} onChange={(e) => pick(values.room, e.target.value as Term)}>
              {TERMS.map((t) => (
                <option key={t.value} value={t.value}>
                  {t.label}
                </option>
              ))}
            </Select>
          </Field>
          <Field label="Academic year" error={errors.year}>
            <Input autoComplete="off" value={values.year} onChange={set('year')} />
          </Field>
          <Field label={`Rate agreed (${desk.currency})`} error={errors.rate}>
            <Input
              inputMode="decimal"
              autoComplete="off"
              className="tabular-nums"
              value={values.rate}
              onChange={set('rate')}
            />
          </Field>
        </div>
      </fieldset>

      {save.error && <Alert title="Could not check in">{save.error}</Alert>}
    </Modal>
  );
}
