import { useEffect, useMemo, useState } from 'react';
import { addToast } from '../lib/toasts.ts';
import {
  checkIn,
  getRooms,
  listResidents,
  type ResidentSummary,
  type Room,
  type Term,
} from '../lib/tauri.ts';
import { useHostel } from '../hooks/use-hostel.ts';
import { Input } from './ui/Input.tsx';
import { Select } from './ui/Select.tsx';
import { Button } from './ui/Button.tsx';
import { SearchInput } from './ui/SearchInput.tsx';

/// A room can be allocated when it isn't under maintenance and still has a
/// free bed — capacity > 1 rooms stay `status: 'occupied'` after their first
/// resident, so `status` alone can't gate room selection.
function hasFreeBed(room: Room): boolean {
  return room.status.toLowerCase() !== 'maintenance' && room.occupied_beds < room.capacity;
}

/// Academic year runs roughly Aug-Jul, so a check-in in Jan-Jul belongs to
/// the year that started the previous August.
function defaultAcademicYear(): string {
  const now = new Date();
  const year = now.getFullYear();
  return now.getMonth() >= 7 ? `${year}/${year + 1}` : `${year - 1}/${year}`;
}

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
  const [nextOfKinPhone, setNextOfKinPhone] = useState('');
  const [rateAgreed, setRateAgreed] = useState('');
  const [term, setTerm] = useState<Term>('semester_1');
  const [academicYear, setAcademicYear] = useState(defaultAcademicYear);

  useEffect(() => {
    if (!hostelId) return;
    getRooms(hostelId)
      .then((data) => setRooms(data.filter(hasFreeBed)))
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

  function rateForTerm(room: Room, t: Term) {
    return t === 'recess' ? room.recess_rate : room.semester_rate;
  }

  function handleRoomChange(value: string) {
    setRoomId(value);
    const room = rooms.find((r) => r.id === Number(value));
    if (room && !rateAgreed) {
      setRateAgreed(rateForTerm(room, term));
    }
  }

  function handleTermChange(value: Term) {
    setTerm(value);
    const room = rooms.find((r) => r.id === Number(roomId));
    if (room && !rateAgreed) {
      setRateAgreed(rateForTerm(room, value));
    }
  }

  async function handleSubmit(event: React.FormEvent) {
    event.preventDefault();
    setError(null);
    const usingExistingResident = residentMode === 'returning' && selectedResidentId !== null;
    if (!roomId || (!usingExistingResident && !fullName.trim())) {
      setError('Room and resident are required.');
      return;
    }
    if (!rateAgreed.trim() || !/^\d+(\.\d+)?$/.test(rateAgreed.trim())) {
      setError('Agreed rate is required and must be a valid decimal amount.');
      return;
    }
    if (!/^\d{4}\/\d{4}$/.test(academicYear.trim())) {
      setError('Academic year must look like "2026/2027".');
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
          next_of_kin_phone: usingExistingResident ? null : nextOfKinPhone.trim() || null,
          rate_agreed: rateAgreed.trim(),
          term,
          academic_year: academicYear.trim(),
        });

      await getRooms(hostelId).then((data) => setRooms(data.filter(hasFreeBed)));
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
      setNextOfKinPhone('');
      setRateAgreed('');
      setTerm('semester_1');
      setAcademicYear(defaultAcademicYear());
      onSuccess?.();
    } catch {
      // Errors are already reported as toasts by the Tauri command wrapper.
    } finally {
      setBusy(false);
    }
  }

  return (
    <form onSubmit={handleSubmit} className="flex flex-col gap-4">
      <Select
        label="Room"
        value={roomId}
        onChange={(e) => handleRoomChange(e.target.value)}
        required
      >
        <option value="">Select an available room</option>
        {rooms.map((room) => (
          <option key={room.id} value={room.id}>
            {room.room_number} · {room.room_type}
            {room.capacity > 1
              ? ` · ${room.capacity - room.occupied_beds}/${room.capacity} beds free`
              : ''}
          </option>
        ))}
      </Select>

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

      {residentMode === 'returning' && (
        <div>
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

      <Input
        label="Resident name"
        type="text"
        value={fullName}
        onChange={(e) => setFullName(e.target.value)}
        required
        placeholder="Full name"
      />

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
        placeholder="Full name"
      />

      <Input
        label="Next of kin phone"
        type="tel"
        value={nextOfKinPhone}
        onChange={(e) => setNextOfKinPhone(e.target.value)}
        placeholder="Emergency contact number"
      />

      <Select
        label="Term"
        value={term}
        onChange={(e) => handleTermChange(e.target.value as Term)}
        required
      >
        <option value="semester_1">Semester 1</option>
        <option value="semester_2">Semester 2</option>
        <option value="recess">Recess / Internship</option>
      </Select>

      <Input
        label="Academic year"
        type="text"
        value={academicYear}
        onChange={(e) => setAcademicYear(e.target.value)}
        required
        placeholder="2026/2027"
      />

      <Input
        label="Agreed rate"
        type="text"
        inputMode="decimal"
        value={rateAgreed}
        onChange={(e) => setRateAgreed(e.target.value)}
        required
        placeholder="0.00"
      />

      {error && (
        <p className="text-caption-sm text-error-text" role="alert">
          {error}
        </p>
      )}

      <Button type="submit" disabled={busy} className="w-full sm:w-auto">
        {busy ? 'Saving…' : 'Check in resident'}
      </Button>
    </form>
  );
}
