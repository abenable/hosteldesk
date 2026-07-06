export type Toast = { id: number; message: string; type: 'error' | 'success' };

let toasts: Toast[] = [];
let listeners: ((toasts: Toast[]) => void)[] = [];
let nextId = 1;

function notify() {
  listeners.forEach((cb) => cb([...toasts]));
}

export function subscribe(cb: (toasts: Toast[]) => void) {
  listeners.push(cb);
  cb([...toasts]);
  return () => {
    listeners = listeners.filter((l) => l !== cb);
  };
}

export function addToast(message: string, type: Toast['type'] = 'error') {
  const toast: Toast = { id: nextId++, message, type };
  toasts = [...toasts, toast];
  notify();
  window.setTimeout(() => removeToast(toast.id), 5000);
}

export function removeToast(id: number) {
  toasts = toasts.filter((t) => t.id !== id);
  notify();
}
