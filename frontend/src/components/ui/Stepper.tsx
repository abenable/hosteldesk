type Step = {
  id: number;
  label: string;
};

type Props = {
  steps: Step[];
  current: number;
};

export function Stepper({ steps, current }: Props) {
  return (
    <ol className="flex w-full items-center gap-2">
      {steps.map((step, i) => {
        const isComplete = step.id < current;
        const isActive = step.id === current;
        const isLast = i === steps.length - 1;
        return (
          <li
            key={step.id}
            className={`flex items-center gap-2 ${isLast ? 'shrink-0' : 'flex-1 min-w-0'}`}
          >
            <span
              className={`flex h-7 w-7 shrink-0 items-center justify-center rounded-full text-caption-sm ${
                isComplete
                  ? 'bg-ink text-on-primary'
                  : isActive
                    ? 'bg-primary text-on-primary'
                    : 'bg-surface-strong text-muted'
              }`}
            >
              {isComplete ? '✓' : step.id}
            </span>
            <span
              className={`truncate text-caption ${
                isActive
                  ? 'text-ink font-medium underline decoration-ink decoration-2 underline-offset-4'
                  : isComplete
                    ? 'text-ink'
                    : 'text-muted'
              }`}
            >
              {step.label}
            </span>
            {!isLast && (
              <span className="mx-1 h-px flex-1 bg-hairline" aria-hidden />
            )}
          </li>
        );
      })}
    </ol>
  );
}
