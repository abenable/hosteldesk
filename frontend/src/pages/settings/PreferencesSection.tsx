import { useState, type FormEvent } from 'react';
import { Alert, Button, Card, Field, Select, Textarea } from '../../components/ui';
import * as api from '../../lib/api';
import { useDesk } from '../../lib/desk';
import { blank, DATE_FORMATS, formatDate, todayIso } from '../../lib/format';
import type { AppPreferences } from '../../lib/types';
import { useAction } from '../../lib/useLoad';
import Section from './Section';

/** Settings → Preferences (admin): date format and receipt footer. Reads `desk.prefs`. */
export default function PreferencesSection() {
  const desk = useDesk();
  return (
    <Section
      title="Dates and receipts"
      description="How dates are written across HostelDesk and on receipts, and the note printed at the bottom of every receipt."
    >
      <PreferencesForm key={desk.hostelId} prefs={desk.prefs} />
    </Section>
  );
}

const toForm = (p: AppPreferences) => ({ dateFormat: p.date_format, footer: p.receipt_footer ?? '' });

function PreferencesForm({ prefs }: { prefs: AppPreferences }) {
  const desk = useDesk();
  const [baseline, setBaseline] = useState(() => toForm(prefs));
  const [values, setValues] = useState(baseline);
  const save = useAction();
  const dirty = values.dateFormat !== baseline.dateFormat || values.footer !== baseline.footer;
  const today = todayIso();

  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (!dirty) return;
    void save.run(async () => {
      const saved = await api.updateAppPreferences({
        hostel_id: desk.hostelId,
        date_format: values.dateFormat,
        language: prefs.language || 'en',
        receipt_footer: blank(values.footer),
      });
      const next = toForm(saved);
      setBaseline(next);
      setValues(next);
      desk.toast({ tone: 'success', title: 'Preferences saved.' });
      desk.reload();
    });
  };

  return (
    <form onSubmit={submit} noValidate>
      <Card
        footer={
          <Button type="submit" disabled={!dirty} loading={save.busy}>
            Save
          </Button>
        }
      >
        <div className="space-y-5">
          <Field label="Date format" hint="Used on every screen and on receipts.">
            <Select
              value={values.dateFormat}
              onChange={(e) => setValues((v) => ({ ...v, dateFormat: e.target.value }))}
            >
              {DATE_FORMATS.map((format) => (
                <option key={format} value={format}>
                  {formatDate(today, format)} ({format})
                </option>
              ))}
            </Select>
          </Field>
          <Field label="Receipt footer" hint="Optional. For example: Thank you for staying with us.">
            <Textarea
              value={values.footer}
              onChange={(e) => setValues((v) => ({ ...v, footer: e.target.value }))}
            />
          </Field>
        </div>
        {save.error && (
          <div className="mt-5">
            <Alert title="Could not save the preferences">{save.error}</Alert>
          </div>
        )}
      </Card>
    </form>
  );
}
