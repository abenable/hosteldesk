import { useEffect, useState } from 'react';
import { Card } from '../../components/ui/Card.tsx';
import { Button } from '../../components/ui/Button.tsx';
import { Select } from '../../components/ui/Select.tsx';
import { Textarea } from '../../components/ui/Textarea.tsx';
import { Modal } from '../../components/ui/Modal.tsx';
import { ReceiptPreview } from '../../components/ReceiptPreview.tsx';
import { useHostel } from '../../hooks/use-hostel.ts';
import { addToast } from '../../lib/toasts.ts';
import {
  getAppPreferences,
  updateAppPreferences,
  type AppPreferences,
} from '../../lib/tauri.ts';

const DATE_FORMATS = [
  { value: 'dd/MM/yyyy', label: 'dd/MM/yyyy' },
  { value: 'MM/dd/yyyy', label: 'MM/dd/yyyy' },
  { value: 'yyyy-MM-dd', label: 'yyyy-MM-dd' },
];

const LANGUAGES = [{ value: 'en', label: 'English' }];

type Props = {
  hostelId: number;
};

export function PreferencesSection({ hostelId }: Props) {
  const { hostel } = useHostel();
  const [prefs, setPrefs] = useState<AppPreferences | null>(null);
  const [loading, setLoading] = useState(true);
  const [dateFormat, setDateFormat] = useState(DATE_FORMATS[0].value);
  const [language, setLanguage] = useState(LANGUAGES[0].value);
  const [receiptFooter, setReceiptFooter] = useState('');
  const [busy, setBusy] = useState(false);
  const [previewOpen, setPreviewOpen] = useState(false);

  useEffect(() => {
    if (!hostelId) return;
    let cancelled = false;
    setLoading(true);
    getAppPreferences({ hostel_id: hostelId })
      .then((p) => {
        if (cancelled) return;
        setPrefs(p);
        setDateFormat(p.date_format);
        setLanguage(p.language);
        setReceiptFooter(p.receipt_footer ?? '');
      })
      .catch(() => {})
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [hostelId]);

  const dirty = prefs && (
    dateFormat !== prefs.date_format ||
    language !== prefs.language ||
    receiptFooter !== (prefs.receipt_footer ?? '')
  );

  async function handleSave(e: React.FormEvent) {
    e.preventDefault();
    if (!hostelId || !dirty) return;
    setBusy(true);
    try {
      const updated = await updateAppPreferences({
        hostel_id: hostelId,
        date_format: dateFormat,
        language,
        receipt_footer: receiptFooter.trim() || null,
      });
      setPrefs(updated);
      addToast('Preferences saved.', 'success');
    } catch {
      // toast surfaced
    } finally {
      setBusy(false);
    }
  }

  if (loading) {
    return (
      <Card shadow className="p-6">
        <div className="flex flex-col gap-3">
          {[...Array(3)].map((_, i) => (
            <div key={i} className="h-14 animate-pulse rounded-full bg-surface-soft" />
          ))}
        </div>
      </Card>
    );
  }

  return (
    <Card shadow className="p-6">
      <h2 className="text-title-md text-ink">Receipt & display</h2>
      <p className="mt-1 text-body-sm text-muted">
        Preferences are shared across your hostel account.
      </p>
      <form onSubmit={handleSave} className="mt-4 flex flex-col gap-4">
        <Select
          label="Date format"
          value={dateFormat}
          onChange={(e) => setDateFormat(e.target.value)}
        >
          {DATE_FORMATS.map((f) => (
            <option key={f.value} value={f.value}>
              {f.label}
            </option>
          ))}
        </Select>

        <Select label="Language" value={language} onChange={(e) => setLanguage(e.target.value)}>
          {LANGUAGES.map((l) => (
            <option key={l.value} value={l.value}>
              {l.label}
            </option>
          ))}
        </Select>

        <Textarea
          label="Receipt footer"
          value={receiptFooter}
          onChange={(e) => setReceiptFooter(e.target.value)}
          rows={3}
          placeholder="Thank you for staying with us!"
        />

        <div className="flex items-center justify-between gap-3">
          <Button type="button" variant="secondary" size="sm" onClick={() => setPreviewOpen(true)}>
            Preview receipt
          </Button>
          <Button type="submit" disabled={busy || !dirty}>
            {busy ? 'Saving…' : 'Save preferences'}
          </Button>
        </div>
      </form>

      <Modal
        open={previewOpen}
        onOpenChange={setPreviewOpen}
        title="Receipt preview"
        description="A sample receipt using your current branding and footer text."
        size="lg"
      >
        <ReceiptPreview
          hostelName={hostel?.name || 'HostelDesk'}
          hostelAddress={hostel?.address}
          hostelPhone={hostel?.phone}
          logoPath={hostel?.branding_logo_path}
          footerText={receiptFooter}
        />
      </Modal>
    </Card>
  );
}
