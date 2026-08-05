import { AppIcon } from "../../../components/AppIcon";
import { LunaMark } from "./PrimaryNavigation";

export function BriefingSkeleton() {
  return (
    <div className="today-skeleton" role="status" aria-live="polite" aria-label="Loading today's briefing">
      <div className="skeleton-greeting" />
      <div className="skeleton-intro"><LunaMark /><div><i /><i /></div></div>
      <div className="skeleton-report"><i /><i /><i /></div>
      <div className="skeleton-report"><i /><i /></div>
      <span className="visually-hidden">Luna is putting your briefing together.</span>
    </div>
  );
}

export function EmptyBriefing() {
  return (
    <section className="today-state-message" aria-labelledby="empty-heading">
      <span className="state-icon"><AppIcon name="check" /></span>
      <h2 id="empty-heading">Everything is under control.</h2>
      <p>Nothing needs your attention right now. You can still ask me to take care of something below.</p>
    </section>
  );
}

export function BriefingError({ message, onRetry }: { message: string; onRetry: () => void }) {
  return (
    <section className="today-state-message" role="alert" aria-labelledby="error-heading">
      <span className="state-icon error"><AppIcon name="alert" /></span>
      <h2 id="error-heading">I could not load today&apos;s household work.</h2>
      <p>{message}</p>
      <button type="button" onClick={onRetry}>Try again</button>
    </section>
  );
}

export function PartialFailure({ title, message }: { title: string; message: string }) {
  return (
    <section className="partial-failure" role="status">
      <AppIcon name="alert" />
      <div><strong>{title}</strong><p>{message}</p></div>
    </section>
  );
}

export function PlaceholderDestination({ destination }: { destination: string }) {
  return (
    <section className="today-state-message placeholder-destination">
      <span className="state-icon"><AppIcon name="clock" /></span>
      <h1>{destination}</h1>
      <p>This destination is part of Luna&apos;s production navigation. Its service-backed experience will be implemented in a focused follow-up.</p>
    </section>
  );
}
