import { useEffect, useRef, useState } from 'react';
import { convertFileSrc } from '@tauri-apps/api/core';
import { Card } from '../../components/ui/Card.tsx';
import { Button } from '../../components/ui/Button.tsx';
import { Input } from '../../components/ui/Input.tsx';
import { RoomsIcon } from '../../components/ui/icons.tsx';
import { addToast } from '../../lib/toasts.ts';
import {
  getHostelProfile,
  updateHostel,
  uploadHostelLogo,
  type HostelProfile,
} from '../../lib/tauri.ts';

const CURRENCY = 'UGX';

type Props = {
  hostelId: number;
  onSaved: () => void;
};

export function BusinessSection({ hostelId, onSaved }: Props) {
  const [profile, setProfile] = useState<HostelProfile | null>(null);
  const [loading, setLoading] = useState(true);
  const [name, setName] = useState('');
  const [address, setAddress] = useState('');
  const [phone, setPhone] = useState('');
  const [logoPath, setLogoPath] = useState<string | undefined>();
  const [uploading, setUploading] = useState(false);
  const [busy, setBusy] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (!hostelId) return;
    let cancelled = false;
    setLoading(true);
    getHostelProfile(hostelId)
      .then((p) => {
        if (cancelled || !p) return;
        setProfile(p);
        setName(p.name);
        setAddress(p.address ?? '');
        setPhone(p.phone ?? '');
        setLogoPath(p.branding_logo_path);
      })
      .catch(() => {})
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [hostelId]);

  const dirty = profile && (
    name !== profile.name ||
    address !== (profile.address ?? '') ||
    phone !== (profile.phone ?? '')
  );

  async function handleFileSelect(e: React.ChangeEvent<HTMLInputElement>) {
    const file = e.target.files?.[0];
    if (!file || !hostelId) return;

    setUploading(true);
    try {
      const buffer = await file.arrayBuffer();
      const bytes = Array.from(new Uint8Array(buffer));
      const path = await uploadHostelLogo({
        hostel_id: hostelId,
        data: bytes,
        filename: file.name,
      });
      setLogoPath(path);
      await updateHostel({
        hostel_id: hostelId,
        name: name.trim(),
        address: address.trim() || undefined,
        phone: phone.trim() || undefined,
        currency: CURRENCY,
        branding_logo_path: path,
      });
      addToast('Logo uploaded.', 'success');
      onSaved();
    } catch {
      // toast surfaced
    } finally {
      setUploading(false);
      if (fileInputRef.current) {
        fileInputRef.current.value = '';
      }
    }
  }

  async function handleSave(e: React.FormEvent) {
    e.preventDefault();
    if (!name.trim() || !hostelId) return;
    setBusy(true);
    try {
      await updateHostel({
        hostel_id: hostelId,
        name: name.trim(),
        address: address.trim() || undefined,
        phone: phone.trim() || undefined,
        currency: CURRENCY,
        branding_logo_path: logoPath,
      });
      addToast('Hostel profile updated.', 'success');
      onSaved();
    } catch {
      // toast already surfaced
    } finally {
      setBusy(false);
    }
  }

  if (loading) {
    return (
      <Card shadow className="p-6">
        <div className="flex flex-col gap-3">
          {[...Array(4)].map((_, i) => (
            <div key={i} className="h-14 animate-pulse rounded-full bg-surface-soft" />
          ))}
        </div>
      </Card>
    );
  }

  return (
    <Card shadow className="p-6">
      <h2 className="text-center text-title-md text-ink">Hostel profile</h2>
      <p className="mt-1 text-center text-body-sm text-muted">Edit your hostel details and branding.</p>

      <div className="mt-5 flex flex-col items-center gap-3">
        <div className="relative flex h-40 w-40 flex-col items-center justify-end gap-2 overflow-hidden rounded-md border border-hairline bg-surface-soft p-3">
          {logoPath ? (
            <img
              src={convertFileSrc(logoPath)}
              alt="Hostel logo"
              className="absolute inset-0 h-full w-full object-contain"
            />
          ) : (
            <RoomsIcon width={56} height={56} className="absolute top-6 text-muted-soft" />
          )}
          <input
            ref={fileInputRef}
            type="file"
            accept="image/png,image/jpeg,image/webp"
            className="hidden"
            onChange={handleFileSelect}
          />
          <div className={logoPath ? 'relative z-10 flex flex-col items-center gap-1 rounded-md bg-canvas/90 px-2 py-1.5 backdrop-blur-sm' : 'flex flex-col items-center gap-1'}>
            <Button
              type="button"
              size="sm"
              className="!h-7"
              onClick={() => fileInputRef.current?.click()}
              disabled={uploading}
            >
              {uploading ? 'Uploading…' : logoPath ? 'Change logo' : 'Upload logo'}
            </Button>
            <p className="text-center text-caption-sm text-muted">PNG, JPG or WebP, max 10 MB.</p>
          </div>
        </div>
      </div>

      <form onSubmit={handleSave} className="mt-6 flex flex-col gap-4">
        <Input label="Hostel name" value={name} onChange={(e) => setName(e.target.value)} required />
        <Input label="Address" value={address} onChange={(e) => setAddress(e.target.value)} />
        <Input label="Phone" type="tel" value={phone} onChange={(e) => setPhone(e.target.value)} />
        <div className="flex justify-end">
          <Button type="submit" disabled={busy || !dirty || !name.trim()}>
            {busy ? 'Saving…' : 'Save changes'}
          </Button>
        </div>
      </form>
    </Card>
  );
}
