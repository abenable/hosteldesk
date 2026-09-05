import type { ReactNode } from 'react';
import * as TabsPrimitive from '@radix-ui/react-tabs';

export type Tab = {
  value: string;
  label: string;
  content: ReactNode;
};

type Props = {
  tabs: Tab[];
  value: string;
  onValueChange: (value: string) => void;
  className?: string;
};

export function Tabs({ tabs, value, onValueChange, className = '' }: Props) {
  return (
    <TabsPrimitive.Root value={value} onValueChange={onValueChange} className={className}>
      <TabsPrimitive.List className="inline-flex items-center gap-1 rounded-full bg-surface-soft p-1">
        {tabs.map((t) => (
          <TabsPrimitive.Trigger
            key={t.value}
            value={t.value}
            className="rounded-full px-4 py-1.5 text-button-sm text-muted transition-[background-color,box-shadow,color] duration-200 ease-out data-[state=active]:bg-canvas data-[state=active]:text-ink data-[state=active]:shadow-card focus:outline-none"
          >
            {t.label}
          </TabsPrimitive.Trigger>
        ))}
      </TabsPrimitive.List>
      {tabs.map((t) => (
        <TabsPrimitive.Content key={t.value} value={t.value} className="mt-4 focus:outline-none">
          {t.content}
        </TabsPrimitive.Content>
      ))}
    </TabsPrimitive.Root>
  );
}
