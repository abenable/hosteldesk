import { createContext } from 'react';

/**
 * The element on the right of the AppShell's content where `<DetailPanel>`
 * renders (through a portal). Null outside the shell: the panel renders inline.
 */
export const DetailSlot = createContext<HTMLElement | null>(null);
