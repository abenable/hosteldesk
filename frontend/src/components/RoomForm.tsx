import { useState } from 'react';
import { Input } from './ui/Input.tsx';
import { Select } from './ui/Select.tsx';
import { Button } from './ui/Button.tsx';
import { addToast } from '../lib/toasts.ts';
import { addRoom, type Room } from '../lib/tauri.ts';
import { FLOOR_OPTIONS, formatRoomNumber } from '../lib/roomNumber.ts';

const ROOM_TYPES = [
  { value: 'single', label: 'Single' },
  { value: 'double', label: 'Double' },
  { value: 'dorm', label: 'Dorm' },
];

type Props = {
  hostelId: number;
  existing: Room[];
  onCreated?: () => void;
};

export function RoomForm({ hostelId, existing, onCreated }: Props) {
  const [floor, setFloor] = useState('A');
  const [roomType, setRoomType] = useState('single');
  const [capacity, setCapacity] = useState('1');
  const [semesterRate, setSemesterRate] = useState('');
  const [recessRate, setRecessRate] = useState('');
  const [busy, setBusy] = useState(false);

  const nextOrdinal = (letter: string) => {
    const count = existing.filter((r) =>
      r.room_number.toUpperCase().startsWith(letter.toUpperCase()),
    ).length;
    return count + 1;
  };

  const suggested = formatRoomNumber(floor, nextOrdinal(floor));
  const [roomNumber, setRoomNumber] = useState(suggested);

  const capacityNum = Number(capacity);
  const rateRe = /^\d+(\.\d+)?$/;
  const valid =
    capacityNum > 0 && rateRe.test(semesterRate.trim()) && rateRe.test(recessRate.trim());

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!valid) {
      addToast('Capacity must be > 0 and both rates valid amounts.', 'error');
      return;
    }
    setBusy(true);
    try {
      await addRoom({
        hostel_id: hostelId,
        room_number: roomNumber.trim() || suggested,
        room_type: roomType,
        capacity: capacityNum,
        floor: floor.charCodeAt(0) - 65,
        semester_rate: semesterRate.trim(),
        recess_rate: recessRate.trim(),
      });
      addToast(`Room ${roomNumber} added.`, 'success');
      onCreated?.();
    } catch {
      // tauri wrapper toasts the error
    } finally {
      setBusy(false);
    }
  }

  return (
    <form onSubmit={handleSubmit} className="flex flex-col gap-4">
      <div className="grid gap-4 sm:grid-cols-2">
        <Select
          label="Floor"
          value={floor}
          onChange={(e) => {
            const f = e.target.value;
            setFloor(f);
            setRoomNumber(formatRoomNumber(f, nextOrdinal(f)));
          }}
        >
          {FLOOR_OPTIONS.map((f) => (
            <option key={f} value={f}>
              Floor {f}
            </option>
          ))}
        </Select>
        <Input
          label="Room number"
          value={roomNumber}
          onChange={(e) => setRoomNumber(e.target.value)}
          placeholder={suggested}
        />
        <Select
          label="Room type"
          value={roomType}
          capitalize
          onChange={(e) => setRoomType(e.target.value)}
        >
          {ROOM_TYPES.map((t) => (
            <option key={t.value} value={t.value}>
              {t.label}
            </option>
          ))}
        </Select>
        <Input
          label="Capacity"
          type="number"
          min={1}
          value={capacity}
          onChange={(e) => setCapacity(e.target.value)}
        />
        <Input
          label="Semester rate (UGX)"
          inputMode="decimal"
          value={semesterRate}
          onChange={(e) => setSemesterRate(e.target.value)}
          placeholder="500000.00"
        />
        <Input
          label="Recess rate (UGX)"
          inputMode="decimal"
          value={recessRate}
          onChange={(e) => setRecessRate(e.target.value)}
          placeholder="200000.00"
        />
      </div>
      <div className="flex justify-end gap-3">
        <Button type="submit" disabled={busy || !valid}>
          {busy ? 'Saving…' : 'Add room'}
        </Button>
      </div>
    </form>
  );
}
