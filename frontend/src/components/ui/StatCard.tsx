import type { ReactNode } from 'react';
import { Card } from './Card.tsx';
import { Badge } from './Badge.tsx';

type Props = {
  label: string;
  value: ReactNode;
  hint?: string;
  trend?: string;
  error?: boolean;
};

export function StatCard({ label, value, hint, trend, error }: Props) {
  return (
    <Card shadow className="p-5">
      <div className="flex items-center justify-between">
        <p className="text-caption text-muted">{label}</p>
        {trend && <Badge className="bg-surface-soft text-muted">{trend}</Badge>}
      </div>
      <p className={`mt-2 text-display-md ${error ? 'text-error-text' : 'text-ink'}`}>{value}</p>
      {hint && <p className="mt-1 text-caption-sm text-muted">{hint}</p>}
    </Card>
  );
}
