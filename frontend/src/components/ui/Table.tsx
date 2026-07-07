import type { ReactNode, ThHTMLAttributes, TdHTMLAttributes, TableHTMLAttributes } from 'react';

type TableProps = TableHTMLAttributes<HTMLTableElement> & {
  children: ReactNode;
};

export function Table({ children, className, ...rest }: TableProps) {
  return (
    <div
      className={`overflow-x-auto rounded-md border border-hairline ${className ?? ''}`}
    >
      <table className="w-full border-collapse text-body-sm" {...rest}>
        {children}
      </table>
    </div>
  );
}

export function Th({ children, className = '', ...rest }: ThHTMLAttributes<HTMLTableCellElement>) {
  return (
    <th
      className={`border-b border-hairline bg-surface-soft px-4 py-3 text-left text-caption text-muted ${className}`}
      {...rest}
    >
      {children}
    </th>
  );
}

export function Td({ children, className = '', ...rest }: TdHTMLAttributes<HTMLTableCellElement>) {
  return (
    <td className={`border-b border-hairline-soft px-4 py-3 text-ink ${className}`} {...rest}>
      {children}
    </td>
  );
}

export function Tr({ children, className = '', ...rest }: React.HTMLAttributes<HTMLTableRowElement>) {
  return (
    <tr className={`transition-colors hover:bg-surface-soft ${className}`} {...rest}>
      {children}
    </tr>
  );
}
