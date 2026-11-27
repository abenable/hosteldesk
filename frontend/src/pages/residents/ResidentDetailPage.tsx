import { useState, type FormEvent } from 'react';
import {
  Alert,
  Avatar,
  Badge,
  Button,
  Card,
  EmptyState,
  Field,
  Input,
  PageHeader,
  Skeleton,
  Stat,
  StatGroup,
  Table,
} from '../../components/ui';
import * as api from '../../lib/api';
import { useDesk } from '../../lib/desk';
import { blank, looksLikeEmail, methodLabel, termLabel } from '../../lib/format';
import { isOwing } from '../../lib/money';
import type { Resident } from '../../lib/types';
import { useAction, useLoad } from '../../lib/useLoad';
import TakePaymentModal from '../payments/TakePaymentModal';
import CheckInModal from './CheckInModal';
import { useCheckOut } from './stay';

/**
 * One resident in full (ui.rs `resident_detail` / `save_resident`): profile
 * and the contact form on the left; figures, the current stay (check out,
 * take payment), every stay and every payment on the right.
 */
export default function ResidentDetailPage({ residentId }: { residentId: number }) {
  const desk = useDesk();
  const detail = useLoad(
    () => api.residentDetail({ hostel_id: desk.hostelId, resident_id: residentId }),
    [desk.hostelId, residentId],
  );
  const stays = useLoad(() => api.activeAllocations({ hostel_id: desk.hostelId }), [desk.hostelId]);
  const checkOut = useCheckOut();
  const [payingId, setPayingId] = useState<number | null>(null);
  const paying = stays.data?.find((s) => s.allocation_id === payingId) ?? null;
  const [checkIn, setCheckIn] = useState({ key: 0, open: false });
  const back = { label: 'Residents', onClick: () => desk.navigate({ page: 'residents' }) };

  const d = detail.data;
  if (detail.loading || !d) {
    return (
      <>
        <PageHeader title="Resident" back={back} />
        {detail.loading ? (
          <div className="grid grid-cols-1 gap-6 xl:grid-cols-3">
            <Card>
              <Skeleton rows={8} />
            </Card>
            <Card className="xl:col-span-2">
              <Skeleton rows={8} />
            </Card>
          </div>
        ) : (
          <Alert title="Could not load this resident" onRetry={detail.reload}>
            {detail.error}
          </Alert>
        )}
      </>
    );
  }

  const r = d.resident;
  const active = d.allocations.filter((a) => a.status === 'active');
  const stayOf = (allocationId: number) => stays.data?.find((s) => s.allocation_id === allocationId);
  const current = active[0] ? stayOf(active[0].id) : undefined;
  const roomOf = (allocationId: number) =>
    d.allocations.find((a) => a.id === allocationId)?.room_number ?? '—';
  const count = d.allocations.length;

  return (
    <>
      <PageHeader
        title={r.full_name}
        back={back}
        description={[
          active[0] ? `Room ${active[0].room_number}` : count ? 'Checked out' : 'Never checked in',
          `${count} ${count === 1 ? 'stay' : 'stays'}`,
          isOwing(d.balance) ? `owes ${desk.formatMoney(d.balance)}` : 'nothing owed',
        ].join(' · ')}
        actions={
          active.length === 0 && (
            <Button icon="user-plus" onClick={() => setCheckIn((c) => ({ key: c.key + 1, open: true }))}>
              Check in again
            </Button>
          )
        }
      />

      <div className="grid grid-cols-1 items-start gap-6 xl:grid-cols-3">
        <ContactForm
          key={r.id}
          resident={r}
          status={active.length > 0 ? 'Checked in' : count > 0 ? 'Checked out' : 'Never checked in'}
        />

        <div className="space-y-6 xl:col-span-2">
          <StatGroup>
            <Stat
              label="Balance"
              value={desk.formatMoney(d.balance)}
              badge={isOwing(d.balance) ? { text: 'Owing', tone: 'danger' } : undefined}
            />
            {current ? (
              <Stat label="Paid" value={desk.formatMoney(current.total_paid)} hint="This stay" />
            ) : (
              <Stat label="Payments" value={d.payments.length} />
            )}
            <Stat label="Stays" value={count} />
          </StatGroup>

          {active.map((a) => {
            const stay = stayOf(a.id);
            return (
              <Alert
                key={a.id}
                tone="info"
                icon="calendar-days"
                title={`Room ${a.room_number} · ${termLabel(a.term)} · ${a.academic_year}`}
                action={
                  <>
                    <Button
                      size="sm"
                      icon="banknotes"
                      loading={stays.loading}
                      disabled={!stay}
                      onClick={() => stay && setPayingId(stay.allocation_id)}
                    >
                      Take payment
                    </Button>
                    <Button
                      size="sm"
                      variant="secondary"
                      onClick={() =>
                        void checkOut({
                          allocation_id: a.id,
                          name: r.full_name,
                          balance: stay?.balance ?? d.balance,
                        })
                      }
                    >
                      Check out
                    </Button>
                  </>
                }
              >
                Since {desk.formatDate(a.created_at)} · rate agreed {desk.formatMoney(a.rate_agreed)}
                {stay &&
                  ` · paid ${desk.formatMoney(stay.total_paid)} · owes ${desk.formatMoney(stay.balance)}`}
              </Alert>
            );
          })}

          <Card title="Stays" flush>
            {count === 0 ? (
              <EmptyState
                icon="home"
                title="Never checked in"
                description="Check them into a room to start a stay."
              />
            ) : (
              <Table
                caption="Stays"
                rows={d.allocations}
                rowKey={(a) => a.id}
                columns={[
                  {
                    header: 'Stay',
                    cell: (a) => (
                      <>
                        <p>Room {a.room_number}</p>
                        <p className="font-normal text-gray-500">
                          {termLabel(a.term)} · {a.academic_year}
                        </p>
                      </>
                    ),
                  },
                  { header: 'Started', cell: (a) => desk.formatDate(a.created_at) },
                  {
                    header: 'Status',
                    cell: (a) =>
                      a.status === 'active' ? <Badge tone="info">Current</Badge> : <Badge>Checked out</Badge>,
                  },
                  { header: 'Rate', align: 'end', cell: (a) => desk.formatMoney(a.rate_agreed) },
                ]}
              />
            )}
          </Card>

          <Card title="Payments" flush>
            {d.payments.length === 0 ? (
              <EmptyState icon="banknotes" title="No payments yet" description="Payments taken for any stay show up here." />
            ) : (
              <Table
                caption="Payments"
                rows={d.payments}
                rowKey={(p) => p.id}
                pageSize={d.payments.length > 10 ? 10 : undefined}
                noun="payments"
                columns={[
                  { header: 'Date', cell: (p) => desk.formatDate(p.payment_date) },
                  {
                    header: 'Method',
                    cell: (p) => (
                      <>
                        <p>{methodLabel(p.payment_method)}</p>
                        {p.notes && <p className="max-w-60 truncate text-gray-500">{p.notes}</p>}
                      </>
                    ),
                  },
                  { header: 'Room', cell: (p) => `Room ${roomOf(p.allocation_id)}` },
                  { header: 'Amount', align: 'end', cell: (p) => desk.formatMoney(p.amount) },
                ]}
              />
            )}
          </Card>
        </div>
      </div>

      <CheckInModal
        key={checkIn.key}
        open={checkIn.open}
        returning={r}
        onClose={() => setCheckIn((c) => ({ ...c, open: false }))}
      />
      <TakePaymentModal stay={paying} onClose={() => setPayingId(null)} />
    </>
  );
}

type Key = 'name' | 'phone' | 'email' | 'id_number' | 'kin' | 'kin_phone';

/**
 * Profile card with the editable contact details (`update_resident`). Seeded
 * once from the loaded resident and keyed by its id, so a background reload
 * never overwrites what is being typed.
 */
function ContactForm({ resident, status }: { resident: Resident; status: string }) {
  const desk = useDesk();
  const [values, setValues] = useState<Record<Key, string>>(() => ({
    name: resident.full_name,
    phone: resident.phone ?? '',
    email: resident.email ?? '',
    id_number: resident.id_number ?? '',
    kin: resident.next_of_kin ?? '',
    kin_phone: resident.next_of_kin_phone ?? '',
  }));
  const [errors, setErrors] = useState<Partial<Record<Key, string>>>({});
  const save = useAction();
  const set = (key: Key) => (e: { target: { value: string } }) => {
    setValues((v) => ({ ...v, [key]: e.target.value }));
    setErrors((errs) => ({ ...errs, [key]: undefined }));
  };

  const submit = (event: FormEvent) => {
    event.preventDefault();
    const found: Partial<Record<Key, string>> = {};
    if (!values.name.trim()) found.name = "Enter the resident's full name.";
    if (values.email.trim() && !looksLikeEmail(values.email)) found.email = 'Enter a valid email address.';
    setErrors(found);
    if (Object.keys(found).length > 0) return;
    void save.run(async () => {
      await api.updateResident({
        hostel_id: desk.hostelId,
        resident_id: resident.id,
        full_name: values.name.trim(),
        phone: blank(values.phone),
        email: blank(values.email),
        id_number: blank(values.id_number),
        next_of_kin: blank(values.kin),
        next_of_kin_phone: blank(values.kin_phone),
      });
      desk.toast({ tone: 'success', title: 'Resident saved.' });
      desk.reload();
    });
  };

  return (
    <form onSubmit={submit} noValidate>
      <Card
        footer={
          <Button type="submit" loading={save.busy}>
            Save details
          </Button>
        }
      >
        <div className="flex items-center gap-4">
          <Avatar name={resident.full_name} size="lg" />
          <div className="min-w-0 text-sm">
            <p className="text-gray-500">Resident since</p>
            <p className="font-medium text-gray-900">{desk.formatDate(resident.created_at)}</p>
            <p className="mt-1.5">
              <Badge>{status}</Badge>
            </p>
          </div>
        </div>
        <div className="mt-6 space-y-4 border-t border-gray-200 pt-6">
          <h2 className="text-base font-semibold text-gray-900">Contact details</h2>
          <Field label="Full name" error={errors.name}>
            <Input autoComplete="off" value={values.name} onChange={set('name')} />
          </Field>
          <Field label="Phone">
            <Input type="tel" autoComplete="off" value={values.phone} onChange={set('phone')} />
          </Field>
          <Field label="Email" error={errors.email}>
            <Input type="email" autoComplete="off" value={values.email} onChange={set('email')} />
          </Field>
          <Field label="ID number">
            <Input autoComplete="off" value={values.id_number} onChange={set('id_number')} />
          </Field>
          <Field label="Next of kin">
            <Input autoComplete="off" value={values.kin} onChange={set('kin')} />
          </Field>
          <Field label="Next-of-kin phone">
            <Input type="tel" autoComplete="off" value={values.kin_phone} onChange={set('kin_phone')} />
          </Field>
          {save.error && <Alert title="Could not save">{save.error}</Alert>}
        </div>
      </Card>
    </form>
  );
}
