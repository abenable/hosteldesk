import { useEffect, useState } from 'react';
import { addToast } from '../lib/toasts.ts';
import {
  addResident,
  createAllocation,
  getRooms,
  type Room,
} from '../lib/tauri.ts';
import { Input } from './ui/Input.tsx';
import { Select } from './ui/Select.tsx';
import { Button } from './ui/Button.tsx';
import { Card } from './ui/Card.tsx';

export function CheckInForm({ onSuccess }: { onSuccess?: () => void }) {
  const [rooms, setRooms] = useState<Room[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [roomId, setRoomId] = useState('');
  const [fullName, setFullName] = useState('');
  const [phone, setPhone] = useState('');
  const [email, setEmail] = useState('');
  const [idNumber, setIdNumber] = useState('');
  const [nextOfKin, setNextOfKin] = useState('');
  const [checkIn, setCheckIn] = useState('');
  const [expectedCheckout, setExpectedCheckout] = useState('');
  const [rateAgreed, setRateAgreed] = useState('');

  async function loadVacantRooms() {
    try {
      const data = await getRooms();
      setRooms(data.filter((r) => r.status.toLowerCase() === 'vacant'));
    } catch {
      setRooms([]);
    }
  }

  useEffect(() => {
    loadVacantRooms();
  }, []);

  async function handleSubmit(event: React.FormEvent) {
    event.preventDefault();
    setError(null);
    if (!roomId || !fullName.trim() || !checkIn || !expectedCheckout) {
      setError('Room, resident name, check-in and expected checkout are required.');
      return;
    }
    if (new Date(expectedCheckout) <= new Date(checkIn)) {
      setError('Expected checkout must be after the check-in date.');
      return;
    }
    if (rateAgreed.trim() && !/^\d+(\.\d+)?$/.test(rateAgreed.trim())) {
      setError('Agreed rate must be a valid decimal amount.');
      return;
    }

    setBusy(true);
    try {
      const residentId = await addResident({
        full_name: fullName.trim(),
        phone: phone.trim() || undefined,
        email: email.trim() || undefined,
        id_number: idNumber.trim() || undefined,
        next_of_kin: nextOfKin.trim() || undefined,
      });

      await createAllocation({
        hostel_id: 1,
        resident_id: residentId,
        room_id: Number(roomId),
        check_in_date: checkIn,
        expected_checkout: expectedCheckout,
        rate_agreed: rateAgreed.trim() || '0',
      });

      await loadVacantRooms();
      addToast('Check-in created successfully', 'success');
      setRoomId('');
      setFullName('');
      setPhone('');
      setEmail('');
      setIdNumber('');
      setNextOfKin('');
      setCheckIn('');
      setExpectedCheckout('');
      setRateAgreed('');
      onSuccess?.();
    } catch {
      // Errors are already reported as toasts by the Tauri command wrapper.
    } finally {
      setBusy(false);
    }
  }

  const today = new Date().toISOString().split('T')[0];

  return (
    <Card shadow className="p-5">
      <form onSubmit={handleSubmit} className="flex flex-col gap-4">
        <div className="grid gap-4 sm:grid-cols-2">
          <div className="sm:col-span-2">
            <Select
              label="Room"
              value={roomId}
              onChange={(e) => setRoomId(e.target.value)}
              required
            >
              <option value="">Select a vacant room</option>
              {rooms.map((room) => (
                <option key={room.id} value={room.id}>
                  {room.room_number} · {room.room_type}
                </option>
              ))}
            </Select>
          </div>

          <div className="sm:col-span-2">
            <Input
              label="Resident name"
              type="text"
              value={fullName}
              onChange={(e) => setFullName(e.target.value)}
              required
              placeholder="Full name"
            />
          </div>

          <Input
            label="Phone"
            type="tel"
            value={phone}
            onChange={(e) => setPhone(e.target.value)}
            placeholder="+1 234 567 890"
          />

          <Input
            label="Email"
            type="email"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            placeholder="resident@example.com"
          />

          <Input
            label="ID number"
            type="text"
            value={idNumber}
            onChange={(e) => setIdNumber(e.target.value)}
            placeholder="Passport / national ID"
          />

          <Input
            label="Next of kin"
            type="text"
            value={nextOfKin}
            onChange={(e) => setNextOfKin(e.target.value)}
            placeholder="Emergency contact"
          />

          <Input
            label="Check-in date"
            type="date"
            value={checkIn}
            min={today}
            onChange={(e) => setCheckIn(e.target.value)}
            required
          />

          <Input
            label="Expected checkout"
            type="date"
            value={expectedCheckout}
            min={checkIn || today}
            onChange={(e) => setExpectedCheckout(e.target.value)}
            required
          />

          <div className="sm:col-span-2">
            <Input
              label="Agreed rate"
              type="text"
              inputMode="decimal"
              value={rateAgreed}
              onChange={(e) => setRateAgreed(e.target.value)}
              required
              placeholder="0.00"
            />
          </div>
        </div>

        {error && (
          <p className="text-caption-sm text-error-text" role="alert">
            {error}
          </p>
        )}

        <Button type="submit" disabled={busy} className="w-full sm:w-auto">
          {busy ? 'Saving…' : 'Check in resident'}
        </Button>
      </form>
    </Card>
  );
}
