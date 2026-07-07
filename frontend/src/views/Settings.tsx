import { Card } from '../components/ui/Card.tsx';
import { Button } from '../components/ui/Button.tsx';
import { Input } from '../components/ui/Input.tsx';
import { addToast } from '../lib/toasts.ts';
import { resetOnboarding, getHostelProfile } from '../lib/onboarding.ts';

export function Settings() {
  const hostel = getHostelProfile();

  const handleRestart = () => {
    resetOnboarding();
    addToast('Onboarding reset. Reloading…', 'success');
    window.location.reload();
  };

  return (
    <div className="flex flex-col gap-6">
      <Card shadow className="p-6">
        <h2 className="text-title-md text-ink">Hostel profile</h2>
        <p className="mt-1 text-body-sm text-muted">
          Basic details set during onboarding.
        </p>
        <div className="mt-4 grid gap-4 sm:grid-cols-2">
          <Input label="Hostel name" defaultValue={hostel?.name ?? ''} readOnly />
          <Input label="Address" defaultValue={hostel?.address ?? ''} readOnly />
          <Input label="Phone" defaultValue={hostel?.phone ?? ''} readOnly />
          <Input label="Currency" defaultValue="UGX" readOnly />
        </div>
      </Card>

      <Card shadow className="p-6">
        <h2 className="text-title-md text-ink">Account</h2>
        <p className="mt-1 text-body-sm text-muted">
          Account management will land here in a future release.
        </p>
      </Card>

      <Card shadow className="p-6">
        <h2 className="text-title-md text-ink">Danger zone</h2>
        <p className="mt-1 text-body-sm text-muted">
          Restart the initial setup wizard. This clears the local onboarding flag
          and reloads the app.
        </p>
        <Button variant="secondary" size="sm" className="mt-4" onClick={handleRestart}>
          Restart onboarding
        </Button>
      </Card>
    </div>
  );
}
