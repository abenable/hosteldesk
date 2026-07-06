import { Card } from '../components/ui/Card.tsx';
import { RoomStatusBoard } from '../components/RoomStatusBoard.tsx';

export function Rooms() {
  return (
    <div className="flex flex-col gap-4">
      <RoomStatusBoard />
      <Card className="p-6 text-center text-body-sm text-muted">
        Room management details coming soon.
      </Card>
    </div>
  );
}
