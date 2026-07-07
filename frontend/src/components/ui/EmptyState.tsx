import type { ReactNode } from 'react';
import { Card } from './Card.tsx';

type Props = {
  title: string;
  description?: string;
  action?: ReactNode;
  icon?: ReactNode;
};

export function EmptyState({ title, description, action, icon }: Props) {
  return (
    <Card className="flex flex-col items-center gap-3 p-10 text-center">
      <span className="flex h-12 w-12 items-center justify-center rounded-full bg-surface-soft text-muted">
        {icon ?? (
          <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2} strokeLinecap="round" strokeLinejoin="round">
            <circle cx="12" cy="12" r="10" />
            <path d="M12 8v4M12 16h.01" />
          </svg>
        )}
      </span>
      <h3 className="text-title-md text-ink">{title}</h3>
      {description && <p className="max-w-sm text-body-sm text-muted">{description}</p>}
      {action && <div className="mt-2">{action}</div>}
    </Card>
  );
}
