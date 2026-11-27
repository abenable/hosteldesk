import { useState, type FormEvent, type ReactNode } from 'react';
import { Alert, Button, Card, Field, Input, Select, Skeleton, Steps, Tabs, Textarea } from '../../components/ui';
import * as api from '../../lib/api';
import { formatMoney } from '../../lib/money';
import { toast } from '../../lib/toast';
import type { RoomClass } from '../../lib/types';
import { useAction, useLoad } from '../../lib/useLoad';
import { ClassForm } from './ClassForm';
import { classLabel, MAX_ROOMS, preview, roomList, roomRange } from './room';

/** Props of {@link RoomSetup}. */
interface RoomSetupProps {
  hostelId: number;
  currency: string;
  /** Every step of the surrounding wizard; the last two are this one's. */
  steps: string[];
  /** Open on the rooms step (the classes step stays one Back away). */
  startOnRooms?: boolean;
  doneLabel: string;
  onDone: () => void;
}

/**
 * The room part of a setup wizard: first the room classes (type and prices),
 * then rooms of each class, added as a run (A01 to A20) or a list.
 * Used by first-run onboarding and by the Rooms page. Admin only.
 */
export function RoomSetup({ hostelId, currency, steps, startOnRooms, doneLabel, onDone }: RoomSetupProps) {
  const classes = useLoad(() => api.roomClasses(hostelId), [hostelId]);
  const [onRooms, setOnRooms] = useState(!!startOnRooms);
  const have = classes.data ?? [];
  const step = steps.length - (onRooms ? 1 : 2);

  return (
    <div className="space-y-6">
      <Steps labels={steps} current={step} />
      {!classes.data ? (
        classes.loading ? (
          <Card>
            <Skeleton rows={4} />
          </Card>
        ) : (
          <Alert title="Could not load room classes" onRetry={classes.reload}>
            {classes.error}
          </Alert>
        )
      ) : onRooms && have.length > 0 ? (
        <RoomsStep hostelId={hostelId} currency={currency} classes={have} onAdded={classes.reload} />
      ) : (
        <ClassesStep hostelId={hostelId} currency={currency} classes={have} onSaved={classes.reload} />
      )}
      <div className="flex items-center justify-between gap-3">
        {onRooms ? (
          <Button variant="secondary" icon="arrow-left" onClick={() => setOnRooms(false)}>
            Room classes
          </Button>
        ) : (
          <span />
        )}
        {onRooms || have.length === 0 ? (
          <Button onClick={onDone}>{doneLabel}</Button>
        ) : (
          <Button onClick={() => setOnRooms(true)}>Next: rooms</Button>
        )}
      </div>
    </div>
  );
}

/** The classes so far, and a form for the next one. */
function ClassesStep({
  hostelId,
  currency,
  classes,
  onSaved,
}: {
  hostelId: number;
  currency: string;
  classes: RoomClass[];
  onSaved: () => void;
}) {
  return (
    <Card
      title="Room classes"
      description="A class is a kind of room: its type and prices. Every room belongs to one class and takes its prices from it."
    >
      <div className="space-y-6">
        {classes.length > 0 && <ClassList classes={classes} currency={currency} />}
        <ClassForm
          hostelId={hostelId}
          currency={currency}
          submitLabel={classes.length === 0 ? 'Add class' : 'Add another class'}
          onSaved={() => {
            toast({ tone: 'success', title: 'Room class added.' });
            onSaved();
          }}
        />
      </div>
    </Card>
  );
}

/** One line per class: name, prices, and how many rooms it has. */
export function ClassList({
  classes,
  currency,
  action,
}: {
  classes: RoomClass[];
  currency: string;
  /** Optional trailing control per class (the Rooms page's Edit). */
  action?: (c: RoomClass) => ReactNode;
}) {
  return (
    <ul className="divide-y divide-gray-200 rounded-3xl border border-gray-200">
      {classes.map((c) => (
        <li key={c.id} className="flex items-center gap-4 px-5 py-3">
          <div className="min-w-0 flex-1">
            <p className="font-medium text-gray-900">{classLabel(c)}</p>
            <p className="text-sm text-gray-500 tabular-nums">
              Semester {formatMoney(c.semester_rate, currency)} · Recess {formatMoney(c.recess_rate, currency)}
            </p>
          </div>
          <span className="text-sm text-gray-500 tabular-nums">
            {c.rooms} {c.rooms === 1 ? 'room' : 'rooms'}
          </span>
          {action?.(c)}
        </li>
      ))}
    </ul>
  );
}

/** Pick a class, type its rooms as a run or a list, add them all at once. */
function RoomsStep({
  hostelId,
  currency,
  classes,
  onAdded,
}: {
  hostelId: number;
  currency: string;
  classes: RoomClass[];
  onAdded: () => void;
}) {
  const [classId, setClassId] = useState(String(classes[0]?.id ?? ''));
  const [mode, setMode] = useState<'range' | 'list'>('range');
  const [prefix, setPrefix] = useState('');
  const [from, setFrom] = useState('1');
  const [to, setTo] = useState('10');
  const [list, setList] = useState('');
  const [shown, setShown] = useState<string | null>(null);
  const save = useAction();

  const numbers = mode === 'range' ? roomRange(prefix, from, to) : roomList(list);
  const problem =
    typeof numbers === 'string'
      ? numbers
      : numbers.length === 0
        ? 'Type at least one room number.'
        : numbers.length > MAX_ROOMS
          ? `Add at most ${MAX_ROOMS} rooms at a time.`
          : null;

  const submit = (event: FormEvent) => {
    event.preventDefault();
    setShown(problem);
    if (problem || typeof numbers === 'string') return;
    void save.run(async () => {
      await api.addRooms({ hostel_id: hostelId, class_id: Number(classId), room_numbers: numbers });
      toast({
        tone: 'success',
        title: numbers.length === 1 ? 'Room added.' : `${numbers.length} rooms added.`,
        message: preview(numbers),
      });
      setList('');
      onAdded();
    });
  };

  return (
    <Card title="Rooms" description="Add the rooms of each class. You can come back to add more at any time.">
      <div className="space-y-6">
        <form onSubmit={submit} noValidate className="space-y-4">
          <Field label="Class">
            <Select value={classId} onChange={(e) => setClassId(e.target.value)}>
              {classes.map((c) => (
                <option key={c.id} value={c.id}>
                  {classLabel(c)} · {formatMoney(c.semester_rate, currency)} a semester
                </option>
              ))}
            </Select>
          </Field>
          <Tabs
            label="How to enter room numbers"
            value={mode}
            onChange={(next) => {
              setMode(next);
              setShown(null);
            }}
            items={[
              { value: 'range', label: 'A run of numbers' },
              { value: 'list', label: 'A list' },
            ]}
          />
          {mode === 'range' ? (
            <div className="grid grid-cols-3 gap-4">
              <Field label="Prefix" hint="The floor or block, e.g. A.">
                <Input value={prefix} onChange={(e) => setPrefix(e.target.value)} />
              </Field>
              <Field label="From" hint="01 keeps two digits.">
                <Input inputMode="numeric" value={from} onChange={(e) => setFrom(e.target.value)} />
              </Field>
              <Field label="To">
                <Input inputMode="numeric" value={to} onChange={(e) => setTo(e.target.value)} />
              </Field>
            </div>
          ) : (
            <Field label="Room numbers" hint="Separate them with commas, spaces or new lines.">
              <Textarea rows={3} value={list} onChange={(e) => setList(e.target.value)} placeholder="A01, A02, B07" />
            </Field>
          )}
          {shown ? (
            <p className="text-sm text-red-700" role="alert">
              {shown}
            </p>
          ) : (
            typeof numbers !== 'string' &&
            numbers.length > 0 && (
              <p className="text-sm text-gray-500">
                {numbers.length} {numbers.length === 1 ? 'room' : 'rooms'}: {preview(numbers)}
              </p>
            )
          )}
          {save.error && <Alert title="Could not add the rooms">{save.error}</Alert>}
          <div className="flex justify-end">
            <Button type="submit" icon="plus" loading={save.busy}>
              {typeof numbers !== 'string' && numbers.length > 1 ? `Add ${numbers.length} rooms` : 'Add room'}
            </Button>
          </div>
        </form>
        <ClassList classes={classes} currency={currency} />
      </div>
    </Card>
  );
}
