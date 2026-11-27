/** Join class names, skipping falsy ones: `cx('p-4', busy && 'opacity-50')`. */
export const cx = (...classes: (string | false | null | undefined)[]) =>
  classes.filter(Boolean).join(' ');
