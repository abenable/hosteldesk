import { Alert, Button } from '../../components/ui';
import AuthLayout from './AuthLayout';

/** HostelDesk could not start (or could not open the hostel): the error and a way to try again. */
export default function BootFailed({
  message,
  onRetry,
  onSignOut,
}: {
  message: string;
  onRetry: () => void;
  /** Offered when a session is open, so the user is never stuck. */
  onSignOut?: () => void;
}) {
  return (
    <AuthLayout title="Could not start HostelDesk">
      <Alert
        title="Something went wrong"
        onRetry={onRetry}
        action={
          onSignOut && (
            <Button size="sm" variant="secondary" onClick={onSignOut}>
              Sign out
            </Button>
          )
        }
      >
        {message}
      </Alert>
    </AuthLayout>
  );
}
