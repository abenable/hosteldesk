import { useEffect, useId, useState, type FormEvent } from 'react';
import { Alert, Button, Card, cx, Field, Icon, Input, Spinner } from '../../components/ui';
import * as api from '../../lib/api';
import { useDesk } from '../../lib/desk';
import { blank } from '../../lib/format';
import type { HostelProfile } from '../../lib/types';
import { useAction } from '../../lib/useLoad';
import Section from './Section';

/** Settings → Business (admin): the hostel's details and the receipt logo. Reads `desk.profile`. */
export default function BusinessSection() {
  const desk = useDesk();
  return (
    <>
      <Section
        title="Business profile"
        description="Your hostel's name and contacts are printed on every receipt."
      >
        <BusinessForm key={desk.profile.id} profile={desk.profile} />
      </Section>
      <Section title="Logo" description="Printed at the top of every receipt. Without one, receipts carry the HostelDesk logo.">
        <LogoCard profile={desk.profile} />
      </Section>
    </>
  );
}

type Key = 'name' | 'address' | 'phone' | 'currency';
type Values = Record<Key, string>;

const toForm = (h: Pick<HostelProfile, Key>): Values => ({
  name: h.name,
  address: h.address ?? '',
  phone: h.phone ?? '',
  currency: h.currency,
});

function BusinessForm({ profile }: { profile: HostelProfile }) {
  const desk = useDesk();
  // Seeded once; `baseline` is what is saved, so Save is enabled only when something changed.
  const [baseline, setBaseline] = useState(() => toForm(profile));
  const [values, setValues] = useState(baseline);
  const [errors, setErrors] = useState<Partial<Record<Key, string>>>({});
  const save = useAction();
  const dirty = (Object.keys(values) as Key[]).some((key) => values[key] !== baseline[key]);
  const set = (key: Key) => (e: { target: { value: string } }) =>
    setValues((v) => ({ ...v, [key]: e.target.value }));

  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (!dirty) return;
    const currency = values.currency.trim().toUpperCase();
    const found: typeof errors = {};
    if (!values.name.trim()) found.name = 'Enter the hostel name.';
    if (!/^[A-Z]{3}$/.test(currency)) found.currency = 'Use a 3-letter currency code, such as UGX.';
    setErrors(found);
    if (Object.keys(found).length > 0) return;
    void save.run(async () => {
      const saved = await api.updateHostel({
        hostel_id: desk.hostelId,
        name: values.name.trim(),
        address: blank(values.address),
        phone: blank(values.phone),
        currency,
        // The latest saved logo (the prop follows every reload).
        branding_logo_path: profile.branding_logo_path,
      });
      const next = toForm(saved);
      setBaseline(next);
      setValues(next);
      desk.toast({ tone: 'success', title: 'Business profile saved.' });
      desk.reload();
    });
  };

  return (
    <form onSubmit={submit} noValidate>
      <Card
        footer={
          <>
            {dirty && (
              <Button
                variant="secondary"
                disabled={save.busy}
                onClick={() => {
                  setValues(baseline);
                  setErrors({});
                  save.setError(null);
                }}
              >
                Discard changes
              </Button>
            )}
            <Button type="submit" disabled={!dirty} loading={save.busy}>
              Save
            </Button>
          </>
        }
      >
        <div className="grid gap-5">
          <Field label="Hostel name" error={errors.name}>
            <Input autoComplete="organization" value={values.name} onChange={set('name')} />
          </Field>
          <Field label="Address">
            <Input autoComplete="street-address" value={values.address} onChange={set('address')} />
          </Field>
          <Field label="Phone">
            <Input type="tel" autoComplete="tel" value={values.phone} onChange={set('phone')} />
          </Field>
          <Field
            label="Currency"
            hint="3 letters. Changing it does not convert amounts."
            error={errors.currency}
          >
            <Input maxLength={3} className="uppercase" value={values.currency} onChange={set('currency')} />
          </Field>
        </div>
        {save.error && (
          <div className="mt-5">
            <Alert title="Could not save the business profile">{save.error}</Alert>
          </div>
        )}
      </Card>
    </form>
  );
}

const MAX_BYTES = 10 * 1024 * 1024;
const fileName = (path: string) => path.split(/[\\/]/).pop() ?? path;

function LogoCard({ profile }: { profile: HostelProfile }) {
  const desk = useDesk();
  const upload = useAction();
  const inputId = useId();
  const [dragging, setDragging] = useState(false);
  // The stored logo is a local file path the CSP will not load; only a file picked
  // in this session can be previewed (as a blob: URL). Otherwise show its file name.
  const [preview, setPreview] = useState<{ url: string; name: string } | null>(null);
  useEffect(
    () => () => {
      if (preview) URL.revokeObjectURL(preview.url);
    },
    [preview],
  );

  const current = profile.branding_logo_path;

  const choose = (file: File | undefined) => {
    if (!file || upload.busy) return;
    if (!/\.(png|jpe?g)$/i.test(file.name)) return upload.setError('Choose a PNG or JPG image.');
    if (file.size > MAX_BYTES) return upload.setError('The logo must be smaller than 10 MB.');
    void upload.run(async () => {
      const data = Array.from(new Uint8Array(await file.arrayBuffer()));
      const path = await api.uploadHostelLogo({ hostel_id: desk.hostelId, data, filename: file.name });
      // As ui.rs: the new logo is saved with the profile at once (the saved values, not unsaved edits).
      await api.updateHostel({
        hostel_id: desk.hostelId,
        name: profile.name,
        address: profile.address,
        phone: profile.phone,
        currency: profile.currency,
        branding_logo_path: path,
      });
      setPreview({ url: URL.createObjectURL(file), name: file.name });
      desk.toast({ tone: 'success', title: 'Logo saved.' });
      desk.reload();
    });
  };

  return (
    <Card>
      <div className="flex items-center gap-4">
        <span className="flex size-16 shrink-0 items-center justify-center overflow-hidden rounded-xl border border-gray-200 bg-gray-50 text-gray-400">
          {preview ? (
            <img src={preview.url} alt="Hostel logo" className="size-full object-contain" />
          ) : (
            <Icon name="building-office" className="size-7" />
          )}
        </span>
        <div className="min-w-0">
          <p className="truncate text-sm font-medium text-gray-900">
            {preview?.name ?? (current ? fileName(current) : 'No logo yet')}
          </p>
          <p className="mt-0.5 text-sm text-gray-500">
            {current ? 'Printed on every receipt.' : 'Receipts print without a logo.'}
          </p>
        </div>
      </div>

      {/* The file input covers the zone, so a dropped file lands on it natively. */}
      <label
        htmlFor={inputId}
        className={cx(
          'relative mt-5 flex cursor-pointer flex-col items-center rounded-xl border border-dashed px-6 py-8 text-center transition-colors has-focus-visible:outline-2 has-focus-visible:outline-offset-2 has-focus-visible:outline-primary-900',
          dragging ? 'border-gray-900 bg-gray-50' : 'border-gray-300 hover:bg-gray-50',
          upload.busy && 'cursor-wait',
        )}
      >
        {upload.busy ? (
          <Spinner size="md" />
        ) : (
          <svg
            aria-hidden="true"
            xmlns="http://www.w3.org/2000/svg"
            fill="none"
            viewBox="0 0 24 24"
            strokeWidth={1.5}
            stroke="currentColor"
            className="size-6 text-gray-500"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              d="M3 16.5v2.25A2.25 2.25 0 0 0 5.25 21h13.5A2.25 2.25 0 0 0 21 18.75V16.5m-13.5-9L12 3m0 0 4.5 4.5M12 3v13.5"
            />
          </svg>
        )}
        <span className="mt-3 text-sm font-medium text-gray-900">
          {upload.busy ? 'Uploading…' : current ? 'Replace the logo' : 'Upload a logo'}
        </span>
        <span className="mt-1 text-sm text-gray-500">Drop an image here, or click to browse.</span>
        <span className="mt-1 text-xs text-gray-500">PNG or JPG, up to 10 MB.</span>
        <input
          id={inputId}
          type="file"
          accept="image/png,image/jpeg"
          className="absolute inset-0 size-full cursor-pointer opacity-0 disabled:cursor-wait"
          disabled={upload.busy}
          onDragEnter={() => setDragging(true)}
          onDragLeave={() => setDragging(false)}
          onDrop={() => setDragging(false)}
          onChange={(e) => {
            choose(e.target.files?.[0]);
            e.target.value = '';
          }}
        />
      </label>

      {upload.error && (
        <div className="mt-5">
          <Alert title="Could not save the logo">{upload.error}</Alert>
        </div>
      )}
    </Card>
  );
}
