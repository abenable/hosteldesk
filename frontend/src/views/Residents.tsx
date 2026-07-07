import { useState } from 'react';
import { Card } from '../components/ui/Card.tsx';
import { EmptyState } from '../components/ui/EmptyState.tsx';
import { Button } from '../components/ui/Button.tsx';
import { ResidentsIcon, PlusIcon } from '../components/ui/icons.tsx';
import type { View } from '../components/Sidebar.tsx';

type Props = { onNavigate?: (v: View) => void };

// ponytail: list_residents command pending — empty state until then.
export function Residents({ onNavigate }: Props) {
  const [query] = useState('');

  return (
    <div className="flex flex-col gap-4">
      <Card shadow className="p-5">
        <div className="flex items-center justify-between gap-4">
          <div>
            <h2 className="text-title-md text-ink">Residents</h2>
            <p className="text-caption-sm text-muted">
              {query ? `Filtering for "${query}"` : 'All resident records'}
            </p>
          </div>
          <Button onClick={() => onNavigate?.('dashboard')}>
            <PlusIcon width={16} height={16} />
            Check in
          </Button>
        </div>
      </Card>

      <EmptyState
        title="No resident records yet"
        description="Resident records will appear here once the backend list command ships. You can still check residents in from the dashboard."
        icon={<ResidentsIcon width={22} height={22} />}
        action={
          <Button variant="secondary" onClick={() => onNavigate?.('dashboard')}>
            Go to dashboard
          </Button>
        }
      />
    </div>
  );
}
