import type { ReactNode } from 'react';
import { Alert, Badge, Button, Card } from '../../components/ui';
import * as api from '../../lib/api';
import { useDesk } from '../../lib/desk';
import { daysUntil, planLabel } from '../../lib/format';
import { useAction } from '../../lib/useLoad';
import Section from './Section';

/** Where renewals go (docs/LICENSING.md; Rust's `desk::SUPPORT_EMAIL` opens it). Plans are set by HostelDesk, not in the app. */
const SUPPORT_EMAIL = 'ableabenaitwe@gmail.com';

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="grid gap-1 py-3">
      <dt className="font-medium text-gray-900">{label}</dt>
      <dd className="text-gray-700">{children}</dd>
    </div>
  );
}

/**
 * Settings → Subscription (everyone): the account's plan, when it ends, and a Renew button that opens the email app on a renewal request
 * (`email_renewal`; a mailto: link would take over the webview). The plan changes on the
 * server once renewed; this computer picks it up on its next sync.
 */
export default function SubscriptionSection() {
  const { license, formatDate } = useDesk();
  const days = license ? daysUntil(license.expires_on) : null;
  const renew = useAction();

  return (
    <Section
      title="Subscription"
      description="Your HostelDesk plan. Renewing early adds the new plan after the current end date, so no days are lost."
    >
      {license && days !== null ? (
        <Card
          footer={
            <Button icon="envelope" loading={renew.busy} onClick={() => void renew.run(api.emailRenewal)}>
              Renew by email
            </Button>
          }
        >
          <dl className="-my-3 divide-y divide-gray-200 text-sm">
            <Row label="Plan">{planLabel(license.plan)}</Row>
            <Row label="Status">
              {days < 0 ? (
                <Badge tone="danger">Ended</Badge>
              ) : days <= 30 ? (
                <Badge tone="warning">Ends soon</Badge>
              ) : (
                <Badge tone="success">Active</Badge>
              )}
            </Row>
            <Row label="Ends on">{formatDate(license.expires_on)}</Row>
            <Row label="Days left">{Math.max(days, 0)}</Row>
            <Row label="Account">{license.account_name}</Row>
            <Row label="To renew">
              Email <span className="font-medium text-gray-900 select-all">{SUPPORT_EMAIL}</span>
            </Row>
          </dl>
          {renew.error && (
            <div className="mt-4">
              <Alert title="Could not open your email app">
                {renew.error} Email {SUPPORT_EMAIL} to renew.
              </Alert>
            </div>
          )}
        </Card>
      ) : (
        <Alert tone="info" title="No plan details yet">
          This computer has not heard from the HostelDesk server yet. It updates on its next sync.
        </Alert>
      )}
    </Section>
  );
}
