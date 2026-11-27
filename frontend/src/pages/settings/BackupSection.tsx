import { Alert, Button, Card, EmptyState, Skeleton, Table } from '../../components/ui';
import * as api from '../../lib/api';
import { useDesk } from '../../lib/desk';
import { useAction, useLoad } from '../../lib/useLoad';
import Section from './Section';

const fileName = (path: string) => path.split(/[\\/]/).pop() ?? path;

/** `…/hosteldesk-20261127-143005.db` → `"2026-11-27T14:30:05"` (local time, as Rust names it), or null. */
function backupTime(path: string): string | null {
  const m = /hosteldesk-(\d{4})(\d{2})(\d{2})-(\d{2})(\d{2})(\d{2})\.db$/i.exec(path);
  return m ? `${m[1]}-${m[2]}-${m[3]}T${m[4]}:${m[5]}:${m[6]}` : null;
}

/** Settings → Backup (admin): back up now, and restore one of the backups (off on cloud-synced desks). */
export default function BackupSection() {
  const desk = useDesk();
  const backups = useLoad(api.listBackups, []);
  // desk.sync.enrolled comes from sign-in; cloud_status also knows about a connect made since.
  const cloud = useLoad(api.cloudStatus, []);
  const enrolled = desk.sync.enrolled || Boolean(cloud.data?.server_url);
  const backup = useAction();
  const list = backups.data ?? [];
  const when = (path: string) => {
    const time = backupTime(path);
    return time ? desk.formatDateTime(time) : fileName(path);
  };
  const latest = list[0];

  const backUp = () =>
    void backup.run(async () => {
      const path = await api.backupDatabase();
      desk.toast({ tone: 'success', title: 'Backup written.', message: path });
      desk.reload();
    });

  const restore = (path: string) =>
    void desk.confirm({
      title: 'Restore this backup?',
      message: (
        <>
          <span className="block font-medium text-gray-900">{when(path)}</span>
          <span className="block">{fileName(path)}</span>
          <span className="mt-3 block">
            Everything recorded since this backup was made will be replaced. This cannot be undone.
            HostelDesk restarts afterwards.
          </span>
        </>
      ),
      confirmLabel: 'Restore',
      tone: 'danger',
      // On success Rust replaces the database and relaunches the app.
      action: () => api.restoreBackup({ backup_path: path }),
    });

  let listBody;
  if (backups.loading) {
    listBody = (
      <Card>
        <Skeleton rows={4} />
      </Card>
    );
  } else if (backups.error && !backups.data) {
    listBody = (
      <Alert title="Could not load the backups" onRetry={backups.reload}>
        {backups.error}
      </Alert>
    );
  } else if (list.length === 0) {
    listBody = (
      <Card>
        <EmptyState
          icon="arrow-down-tray"
          title="No backups yet"
          description="Back up now to keep a copy of everything on this computer."
        />
      </Card>
    );
  } else {
    listBody = (
      <Card flush>
        <Table
          caption="Backups, newest first"
          rows={list}
          rowKey={(path) => path}
          pageSize={8}
          noun="backups"
          columns={[
            {
              header: 'Backup',
              cell: (path) => (
                <div className="min-w-0">
                  <p className="font-medium text-gray-900 tabular-nums">{when(path)}</p>
                  <p className="truncate text-sm font-normal text-gray-500">{fileName(path)}</p>
                </div>
              ),
            },
            ...(enrolled
              ? []
              : [
                  {
                    header: <span className="sr-only">Actions</span>,
                    align: 'end' as const,
                    cell: (path: string) => (
                      <Button size="sm" variant="secondary" icon="arrow-path" onClick={() => restore(path)}>
                        Restore
                      </Button>
                    ),
                  },
                ]),
          ]}
        />
      </Card>
    );
  }

  return (
    <>
      <Section
        title="Back up"
        description="Writes a copy of everything on this computer to Documents › HostelDesk › backups. Copy it somewhere safe, such as a USB stick."
      >
        <Card
          footer={
            <Button icon="arrow-down-tray" loading={backup.busy} onClick={backUp}>
              Back up now
            </Button>
          }
        >
          <p className="text-sm text-gray-500">Last backup</p>
          <p className="mt-1 text-base font-semibold text-gray-900 tabular-nums">
            {backups.loading ? '…' : latest ? when(latest) : 'Never'}
          </p>
          {list.length > 0 && (
            <p className="mt-1 text-sm text-gray-500">
              {list.length} {list.length === 1 ? 'backup' : 'backups'} on this computer
            </p>
          )}
          {backup.error && (
            <div className="mt-5">
              <Alert title="Could not back up">{backup.error}</Alert>
            </div>
          )}
        </Card>
      </Section>
      <Section
        title="Restore"
        description="Replace everything on this computer with a backup. HostelDesk restarts afterwards."
      >
        {enrolled && (
          <Alert tone="info" icon="cloud" title="Restoring is turned off">
            This computer syncs with the cloud, so restoring a backup is turned off. Your data is
            already kept in the cloud.
          </Alert>
        )}
        {listBody}
      </Section>
    </>
  );
}
