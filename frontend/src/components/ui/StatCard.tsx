import type { ReactNode } from 'react';
import { Card } from './Card.tsx';

type Props = {
  label: string;
  value: ReactNode;
  hint?: ReactNode;
  icon?: ReactNode;
};

export function StatCard({ label, value, hint, icon }: Props) {
  return (
    <Card shadow className="p-5">
      <div className="flex items-start justify-between">
        <p className="text-uppercase-tag text-muted">{label}</p>
        {icon && <span className="text-muted">{icon}</span>}
      </div>
      <p className="mt-2 text-display-lg text-ink">{value}</p>
      {hint && <p className="mt-1 text-caption-sm text-muted">{hint}</p>}
    </Card>
  );
}
