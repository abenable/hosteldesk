import { useEffect, useState } from 'react';
import { Card } from '../../components/ui/Card.tsx';
import { Button } from '../../components/ui/Button.tsx';
import { Modal } from '../../components/ui/Modal.tsx';
import { addToast } from '../../lib/toasts.ts';
import { resetOnboarding } from '../../lib/onboarding.ts';
import {
  backupDatabase,
  listBackups,
  restoreBackup,
} from '../../lib/tauri.ts';

export function DataSecuritySection() {
  const [backing, setBacking] = useState(false);
  const [backups, setBackups] = useState<string[]>([]);
  const [loadingBackups, setLoadingBackups] = useState(true);
  const [selectedBackup, setSelectedBackup] = useState<string | null>(null);
  const [confirmReset, setConfirmReset] = useState(false);

  useEffect(() => {
    loadBackups();
  }, []);

  async function loadBackups() {
    setLoadingBackups(true);
    try {
      const list = await listBackups();
      setBackups(list);
    } catch {
      // toast surfaced
    } finally {
      setLoadingBackups(false);
    }
  }

  async function handleBackup() {
    setBacking(true);
    try {
      const path = await backupDatabase();
      addToast(`Backed up to ${path}`, 'success');
      await loadBackups();
    } catch {
      // toast surfaced
    } finally {
      setBacking(false);
    }
  }

  async function handleRestore(path: string) {
    try {
      await restoreBackup({ backup_path: path });
    } catch {
      // toast surfaced
    }
  }

  function handleReset() {
    setConfirmReset(false);
    resetOnboarding();
    addToast('Onboarding reset. Reloading…', 'success');
    window.location.reload();
  }

  return (
    <div className="flex flex-col gap-6">
      <Card shadow className="p-6">
        <h2 className="text-title-md text-ink">Backup</h2>
        <p className="mt-1 text-body-sm text-muted">
          Create a copy of your database in Documents/HostelDesk/backups/.
        </p>
        <div className="mt-4">
          <Button variant="secondary" size="sm" onClick={handleBackup} disabled={backing}>
            {backing ? 'Backing up…' : 'Back up now'}
          </Button>
        </div>
      </Card>

      <Card shadow className="p-6">
        <h2 className="text-title-md text-ink">Restore</h2>
        <p className="mt-1 text-body-sm text-muted">
          Pick a previous backup to restore. The app will restart afterwards.
        </p>

        {loadingBackups ? (
          <div className="mt-4 space-y-2">
            {[...Array(3)].map((_, i) => (
              <div key={i} className="h-10 animate-pulse rounded-md bg-surface-soft" />
            ))}
          </div>
        ) : backups.length === 0 ? (
          <p className="mt-4 text-body-sm text-muted">No backups found.</p>
        ) : (
          <ul className="mt-4 flex flex-col gap-2">
            {backups.map((path) => {
              const name = path.split(/[/\\]/).pop() ?? path;
              return (
                <li
                  key={path}
                  className="flex items-center justify-between rounded-sm border border-hairline px-3 py-2"
                >
                  <span className="text-body-sm text-ink">{name}</span>
                  <Button
                    variant="secondary"
                    size="sm"
                    onClick={() => setSelectedBackup(path)}
                  >
                    Restore
                  </Button>
                </li>
              );
            })}
          </ul>
        )}
      </Card>

      <Card shadow className="p-6">
        <h2 className="text-title-md text-ink">Danger zone</h2>
        <p className="mt-1 text-body-sm text-muted">
          Restart the initial setup wizard. This does not delete your hostel data — it only re-runs the wizard.
        </p>
        <div className="mt-4">
          <Button variant="secondary" size="sm" onClick={() => setConfirmReset(true)}>
            Restart onboarding
          </Button>
        </div>
      </Card>

      <Modal
        open={!!selectedBackup}
        onOpenChange={() => setSelectedBackup(null)}
        title="Restore backup?"
        description={`The app will restart and replace the current database with ${
          selectedBackup?.split(/[/\\]/).pop() ?? ''
        }. This cannot be undone.`}
      >
        <div className="flex justify-end gap-3">
          <Button variant="tertiary" onClick={() => setSelectedBackup(null)}>
            Cancel
          </Button>
          <Button
            variant="secondary"
            onClick={() => selectedBackup && handleRestore(selectedBackup)}
          >
            Restore & restart
          </Button>
        </div>
      </Modal>

      <Modal
        open={confirmReset}
        onOpenChange={setConfirmReset}
        title="Restart onboarding?"
        description="This will reload the app and show the setup wizard again."
      >
        <div className="flex justify-end gap-3">
          <Button variant="tertiary" onClick={() => setConfirmReset(false)}>
            Cancel
          </Button>
          <Button variant="secondary" onClick={handleReset}>
            Restart
          </Button>
        </div>
      </Modal>
    </div>
  );
}
