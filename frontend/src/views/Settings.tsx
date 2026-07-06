import { Card } from '../components/ui/Card.tsx';
import { Button } from '../components/ui/Button.tsx';
import { addToast } from '../lib/toasts.ts';
import { resetOnboarding } from '../lib/onboarding.ts';

export function Settings() {
  const handleRestart = () => {
    resetOnboarding();
    addToast('Onboarding reset. Reloading…', 'success');
    window.location.reload();
  };

  return (
    <Card className="flex flex-col gap-4 p-6">
      <h2 className="text-title-md text-ink">Settings</h2>
      <p className="text-body-sm text-muted">
        Configuration options will appear here in a future release.
      </p>
      <div className="border-t border-hairline pt-4">
        <h3 className="text-title-sm text-ink">Onboarding</h3>
        <p className="mt-1 text-body-sm text-muted">
          Restart the initial setup wizard. This clears the local onboarding
          flag and reloads the app.
        </p>
        <Button variant="secondary" size="sm" className="mt-3" onClick={handleRestart}>
          Restart onboarding
        </Button>
      </div>
    </Card>
  );
}
