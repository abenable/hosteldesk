import { useEffect, useMemo, useState } from 'react';
import { addToast } from '../lib/toasts.ts';
import {
  checkIn,
  getRooms,
  listResidents,
  type ResidentSummary,
  type Room,
} from '../lib/tauri.ts';
import { useHostel } from '../hooks/use-hostel.ts';
import { Input } from './ui/Input.tsx';
import { Select } from './ui/Select.tsx';
import { Button } from './ui/Button.tsx';
import { Card } from './ui/Card.tsx';
import { SearchInput } from './ui/SearchInput.tsx';

export function CheckInForm({ onSuccess }: { onSuccess?: () => void }) {
  const { hostelId } = useHostel();
  const [rooms, setRooms] = useState<Room[]>([]);
  const [residents, setResidents] = useState<ResidentSummary[]>([]);
  const [residentMode, setResidentMode] = useState<'new' | 'returning'>('new');
  const [residentQuery, setResidentQuery] = useState('');
  const [selectedResidentId, setSelectedResidentId] = useState<number | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [roomId, setRoomId] = useState('');
  const [fullName, setFullName] = useState('');
  const [phone, setPhone] = useState('');
  const [email, setEmail] = useState('');
  const [idNumber, setIdNumber] = useState('');
  const [nextOfKin, setNextOfKin] = useState('');
  const [checkInDate, setCheckInDate] = useState('');
  const [expectedCheckout, setExpectedCheckout] = useState('');
  const [rateAgreed, setRateAgreed] = useState('');

  useEffect(() => {
    if (!hostelId) return;
    getRooms(hostelId)
      .then((data) => setRooms(data.filter((r) => r.status.toLowerCase() === 'vacant')))
      .catch(() => setRooms([]));
    listResidents({ hostel_id: hostelId })
      .then(setResidents)
      .catch(() => setResidents([]));
  }, [hostelId]);

  const residentMatches = useMemo(() => {
    const query = residentQuery.trim().toLowerCase();
    if (!query) return residents.slice(0, 5);
    return residents
      .filter((resident) =>
        resident.full_name.toLowerCase().includes(query) ||
        (resident.phone ?? '').toLowerCase().includes(query) ||
        (resident.email ?? '').toLowerCase().includes(query)
      )
      .slice(0, 5);
  }, [residentQuery, residents]);

  function selectResident(resident: ResidentSummary) {
    setSelectedResidentId(resident.id);
    setFullName(resident.full_name);
    setPhone(resident.phone ?? '');
    setEmail(resident.email ?? '');
    setResidentQuery(resident.full_name);
  }

  function handleResidentMode(mode: 'new' | 'returning') {
    setResidentMode(mode);
    setSelectedResidentId(null);
    if (mode === 'new') setResidentQuery('');
  }

  function handleRoomChange(value: string) {
    setRoomId(value);
    const room = rooms.find((r) => r.id === Number(value));
    if (room && !rateAgreed) {
      setRateAgreed(room.rate_per_period);
    }
  }

  async function handleSubmit(event: React.FormEvent) {
    event.preventDefault();
    setError(null);
    const usingExistingResident = residentMode === 'returning' && selectedResidentId !== null;
    if (!roomId || !checkInDate || !expectedCheckout || (!usingExistingResident && !fullName.trim())) {
      setError('Room, resident, check-in and expected checkout are required.');
      return;
    }
    if (new Date(expectedCheckout) <= new Date(checkInDate)) {
      setError('Expected checkout must be after the check-in date.');
      return;
    }
    if (!rateAgreed.trim() || !/^\d+(\.\d+)?$/.test(rateAgreed.trim())) {
      setError('Agreed rate is required and must be a valid decimal amount.');
      return;
    }

    setBusy(true);
    try {
        await checkIn({
          hostel_id: hostelId,
          room_id: Number(roomId),
          resident_id: usingExistingResident ? selectedResidentId : null,
          full_name: usingExistingResident ? null : fullName.trim(),
          phone: usingExistingResident ? null : phone.trim() || null,
          email: usingExistingResident ? null : email.trim() || null,
          id_number: usingExistingResident ? null : idNumber.trim() || null,
          next_of_kin: usingExistingResident ? null : nextOfKin.trim() || null,
          check_in_date: checkInDate,
          expected_checkout: expectedCheckout,
          rate_agreed: rateAgreed.trim(),
        });

      await getRooms(hostelId).then((data) => setRooms(data.filter((r) => r.status.toLowerCase() === 'vacant')));
      addToast('Check-in created successfully', 'success');
      setRoomId('');
      setResidentMode('new');
      setResidentQuery('');
      setSelectedResidentId(null);
      setFullName('');
      setPhone('');
      setEmail('');
      setIdNumber('');
      setNextOfKin('');
      setCheckInDate('');
      setExpectedCheckout('');
      setRateAgreed('');
      onSuccess?.();
    } catch {
      // Errors are already reported as toasts by the Tauri command wrapper.
    } finally {
      setBusy(false);
    }
  }

  const today = new Date().toLocaleDateString('en-CA');

  return (
    <Card shadow className="p-5">
      <form onSubmit={handleSubmit} className="flex flex-col gap-4">
        <div className="grid gap-4 sm:grid-cols-2">
          <div className="sm:col-span-2">
            <Select
              label="Room"
              value={roomId}
              onChange={(e) => handleRoomChange(e.target.value)}
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
            <div role="group" aria-label="Resident type" className="flex flex-wrap gap-2">
              <Button
                type="button"
                variant={residentMode === 'new' ? 'secondary' : 'tertiary'}
                size="sm"
                onClick={() => handleResidentMode('new')}
              >
                New resident
              </Button>
              <Button
                type="button"
                variant={residentMode === 'returning' ? 'secondary' : 'tertiary'}
                size="sm"
                onClick={() => handleResidentMode('returning')}
              >
                Returning resident
              </Button>
            </div>
          </div>

          {residentMode === 'returning' && (
            <div className="sm:col-span-2">
              <label className="text-caption text-muted" htmlFor="returning-resident-search">
                Search residents
              </label>
              <SearchInput
                id="returning-resident-search"
                value={residentQuery}
                onChange={(event) => {
                  setResidentQuery(event.target.value);
                  setSelectedResidentId(null);
                }}
                placeholder="Search by name, phone, or email"
              />
              <div className="mt-2 flex flex-col gap-2">
                {residentMatches.map((resident) => (
                  <button
                    key={resident.id}
                    type="button"
                    onClick={() => selectResident(resident)}
                    className={`rounded-md border px-3 py-2 text-left text-body-sm transition-colors ${
                      selectedResidentId === resident.id
                        ? 'border-ink bg-surface-soft text-ink'
                        : 'border-hairline bg-canvas text-ink hover:bg-surface-soft'
                    }`}
                  >
                    <span className="font-medium">{resident.full_name}</span>
                    <span className="ml-2 text-caption-sm text-muted">
                      {resident.room_number ? `Last room ${resident.room_number}` : 'No previous room'}
                    </span>
                  </button>
                ))}
                {residentMatches.length === 0 && (
                  <p className="text-caption-sm text-muted">No matching residents.</p>
                )}
              </div>
            </div>
          )}

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
            value={checkInDate}
            min={today}
            onChange={(e) => setCheckInDate(e.target.value)}
            required
          />

          <Input
            label="Expected checkout"
            type="date"
            value={expectedCheckout}
            min={checkInDate || today}
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
