import { useEffect, type ReactNode } from 'react';
import { Alert, Badge, Button, Card, EmptyState, Skeleton } from '../../components/ui';
import * as api from '../../lib/api';
import { useDesk } from '../../lib/desk';
import type { CloudStatus } from '../../lib/types';
import { useLoad } from '../../lib/useLoad';
import CloudConnectForm from '../auth/CloudConnect';
import Section from './Section';

/** SQLite `CURRENT_TIMESTAMP` ("2026-11-27 09:30:00") is UTC without a zone; mark it so it shows in local time. */
const utc = (value: string) =>
  /^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}$/.test(value) ? `${value.replace(' ', 'T')}Z` : value;

/** `"check_in"` → `"Check in"`. */
const changeLabel = (type: string) => {
  const text = type.replace(/_/g, ' ');
  return text.charAt(0).toUpperCase() + text.slice(1);
};

const TITLE = 'Cloud sync';

/**
 * Settings → Cloud (everyone): connection details, Sync now and the changes the
 * cloud refused; on a desk that is not connected, the connect form (admins only).
 */
export default function CloudSection() {
  const desk = useDesk();
  const status = useLoad(api.cloudStatus, []);
  // A finished sync changes the pending count and issues; refresh without a skeleton flash.
  const lastAt = desk.sync.lastAt?.getTime();
  const { reload } = status;
  useEffect(() => reload(), [lastAt, reload]);

  if (status.loading)
    return (
      <Section title={TITLE}>
        <Card>
          <Skeleton rows={4} />
        </Card>
      </Section>
    );
  if (!status.data)
    return (
      <Section title={TITLE}>
        <Alert title="Could not load the cloud status" onRetry={reload}>
          {status.error}
        </Alert>
      </Section>
    );
  if (!status.data.server_url) return <NotConnected />;
  return <Connected status={status.data} serverUrl={status.data.server_url} />;
}

function NotConnected() {
  const desk = useDesk();
  return (
    <Section
      title="Connect to the cloud"
      description="This computer is not connected to the cloud. Connecting uploads this hostel so other computers in your organization can share it."
    >
      {desk.isAdmin ? (
        <Card>
          <CloudConnectForm onConnected={() => desk.reload()} />
        </Card>
      ) : (
        <Alert tone="info" icon="cloud" title="Not connected">
          Ask an administrator to connect this computer to your HostelDesk account.
        </Alert>
      )}
    </Section>
  );
}

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="grid gap-1 py-3 sm:grid-cols-3 sm:gap-4">
      <dt className="font-medium text-gray-900">{label}</dt>
      <dd className="break-words text-gray-700 sm:col-span-2">{children}</dd>
    </div>
  );
}

function Connected({ status, serverUrl }: { status: CloudStatus; serverUrl: string }) {
  const desk = useDesk();
  const { sync } = desk;
  const rejected = status.issues.filter((issue) => issue.rejected).length;
  const warnings = status.issues.length - rejected;

  return (
    <>
      <Section
        title={TITLE}
        description="This computer sends its changes to the cloud and receives the other computers' changes every 30 seconds."
      >
        <Card
          footer={
            <Button icon="arrow-path" loading={sync.busy} onClick={desk.syncNow}>
              Sync now
            </Button>
          }
        >
          <dl className="-my-3 divide-y divide-gray-200 text-sm">
            <Row label="Server">{serverUrl}</Row>
            <Row label="Receipt prefix">
              {status.device_prefix ?? '—'}
            </Row>
            <Row label="Waiting to send">
              {status.pending === 0
                ? 'Nothing, all changes are sent'
                : `${status.pending} ${status.pending === 1 ? 'change' : 'changes'}`}
            </Row>
            <Row label="Last sync">
              {sync.busy
                ? 'Syncing…'
                : sync.lastAt
                  ? desk.formatDateTime(sync.lastAt.toISOString())
                  : 'Not yet since you signed in'}
            </Row>
          </dl>
          {sync.error && (
            <div className="mt-6">
              <Alert tone="warning" title="The last sync failed. It retries every 30 seconds.">
                {sync.error}
              </Alert>
            </div>
          )}
        </Card>
      </Section>
      <Section
        title="Needs attention"
        description={
          status.issues.length === 0
            ? 'Changes from this computer that the cloud refused or warned about.'
            : `${rejected} not accepted · ${warnings} ${warnings === 1 ? 'warning' : 'warnings'}. Check these records and correct them by hand if needed.`
        }
      >
        <Card flush={status.issues.length > 0}>
          {status.issues.length === 0 ? (
            <EmptyState
              icon="check-circle"
              title="Nothing needs attention"
              description="The cloud accepted every change from this computer."
            />
          ) : (
            <ul className="divide-y divide-gray-200">
              {status.issues.map((issue, i) => (
                <li key={i} className="px-6 py-4">
                  <div className="flex flex-wrap items-center gap-x-3 gap-y-1">
                    <p className="text-sm font-medium text-gray-900">{changeLabel(issue.command_type)}</p>
                    <Badge tone={issue.rejected ? 'danger' : 'warning'}>
                      {issue.rejected ? 'Not accepted' : 'Warning'}
                    </Badge>
                    <time className="ms-auto text-sm text-gray-500 tabular-nums">
                      {desk.formatDateTime(utc(issue.created_at))}
                    </time>
                  </div>
                  <p className="mt-1 text-sm text-pretty text-gray-600">{issue.message}</p>
                </li>
              ))}
            </ul>
          )}
        </Card>
      </Section>
    </>
  );
}
