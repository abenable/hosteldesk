import type { RetoucheConfig } from 'retouche-dev/config';

// Dev-only overlay (retouche.dev): flags call sites that override the
// typography of these components/ui components. Left out because they set no
// type: BrandMark, Icon, Skeleton, StatGroup, ConfirmDialog (Modal + Buttons, no classes).
// A type, not defineConfig(): its module would pull zod (~80 kB) into release builds.
export default {
  components: [
    'Alert',
    'Avatar',
    'Badge',
    'Button',
    'Card',
    'Checkbox',
    'DetailPanel',
    'DetailSection',
    'Dropdown',
    'EmptyState',
    'Field',
    'Input',
    'MiniStats',
    'Modal',
    'NameCell',
    'PageHeader',
    'SearchInput',
    'Select',
    'Spinner',
    'Stat',
    'Steps',
    'Table',
    'Tabs',
    'Textarea',
    'Toaster',
    'Toggle',
  ],
} satisfies RetoucheConfig;
