import { FormEvent, useState } from "react";
import "./App.css";
import type { AccountService, HouseholdSession } from "./account/accountService";
import { AccountFlow } from "./account/AccountFlow";
import { TrustedDeviceFlow, TrustedDeviceUnlock } from "./trusted-device/TrustedDeviceFlow";
import type { TrustedDeviceService } from "./trusted-device/trustedDeviceService";
import { TrustedDevicesOptions } from "./trusted-device/TrustedDevicesOptions";

const destinations = ["Luna", "To do", "Cabinet", "History", "Options"] as const;
type TrustedDeviceMode = "first" | "recovery";

type AppProps = {
  accountService: AccountService;
  trustedDeviceService: TrustedDeviceService;
};

export default function App({ accountService, trustedDeviceService }: AppProps) {
  const [session, setSession] = useState<HouseholdSession | null>(null);
  const [pendingTrustedSession, setPendingTrustedSession] = useState<{
    session: HouseholdSession;
    mode: TrustedDeviceMode;
  } | null>(null);
  const [pendingUnlockSession, setPendingUnlockSession] = useState<{
    session: HouseholdSession;
    pendingKeyCoordination?: { keyEnvelope: string; keyEpoch: number };
  } | null>(null);
  const [accountEntry, setAccountEntry] = useState<"registration" | "signIn">("registration");
  const [draft, setDraft] = useState("");
  const [messages, setMessages] = useState<string[]>([]);
  const [activeDestination, setActiveDestination] = useState<(typeof destinations)[number]>("Luna");

  const submitMessage = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const message = draft.trim();
    if (!message) return;
    setMessages((current) => [...current, message]);
    setDraft("");
  };

  const signOut = async () => {
    if (session) await trustedDeviceService.lockDevice(session.householdId);
    await accountService.signOut();
    setAccountEntry("signIn");
    setSession(null);
  };

  const authenticate = async (authenticatedSession: HouseholdSession) => {
    if (await trustedDeviceService.isCurrentDeviceTrusted(authenticatedSession.householdId)) {
      const currentPublicKey = await trustedDeviceService.currentDevicePublicKey(authenticatedSession.householdId);
      const coordination = await accountService.getTrustedDeviceKeyCoordination(currentPublicKey);
      if (coordination.status !== "active") {
        await trustedDeviceService.forgetCurrentDevice(authenticatedSession.householdId);
        setPendingTrustedSession({ session: authenticatedSession, mode: "recovery" });
        return;
      }
      const localKeyEpoch = await trustedDeviceService.currentKeyEpoch(authenticatedSession.householdId);
      if (coordination.keyEpoch < localKeyEpoch) {
        throw new Error("The service returned a stale Household key epoch.");
      }
      const isUnlocked = await trustedDeviceService.isCurrentDeviceUnlocked(authenticatedSession.householdId);
      if (coordination.keyEpoch > localKeyEpoch && isUnlocked) {
        await trustedDeviceService.applyRotatedDeviceEnvelope(
          authenticatedSession.householdId,
          coordination.keyEnvelope,
          coordination.keyEpoch,
        );
      }
      if (isUnlocked) {
        setSession(authenticatedSession);
      } else {
        setPendingUnlockSession({
          session: authenticatedSession,
          pendingKeyCoordination: coordination.keyEpoch > localKeyEpoch
            ? { keyEnvelope: coordination.keyEnvelope, keyEpoch: coordination.keyEpoch }
            : undefined,
        });
      }
      return;
    }
    const authenticatorStatus = await accountService.getAuthenticatorStatus();
    setPendingTrustedSession({
      session: authenticatedSession,
      mode: authenticatorStatus === "unenrolled" ? "first" : "recovery",
    });
  };

  if (pendingUnlockSession) {
    return <TrustedDeviceUnlock
      session={pendingUnlockSession.session}
      trustedDeviceService={trustedDeviceService}
      onUnlocked={async () => {
        if (pendingUnlockSession.pendingKeyCoordination) {
          await trustedDeviceService.applyRotatedDeviceEnvelope(
            pendingUnlockSession.session.householdId,
            pendingUnlockSession.pendingKeyCoordination.keyEnvelope,
            pendingUnlockSession.pendingKeyCoordination.keyEpoch,
          );
        }
        setSession(pendingUnlockSession.session);
        setPendingUnlockSession(null);
      }}
    />;
  }

  if (pendingTrustedSession) {
    return <TrustedDeviceFlow
      accountService={accountService}
      mode={pendingTrustedSession.mode}
      session={pendingTrustedSession.session}
      trustedDeviceService={trustedDeviceService}
      onTrusted={() => {
        setSession(pendingTrustedSession.session);
        setPendingTrustedSession(null);
      }}
    />;
  }

  if (!session) {
    return <AccountFlow
      accountService={accountService}
      initialStep={accountEntry}
      onAuthenticated={authenticate}
    />;
  }

  const initials = session.organiserName
    .split(/\s+/)
    .map((part) => part[0])
    .join("")
    .slice(0, 2)
    .toUpperCase();

  return (
    <div className="luna-shell">
      <aside className="sidebar">
        <div className="brand"><span aria-hidden="true">L</span><strong>Luna</strong></div>
        <button className="new-conversation" type="button">＋ New conversation</button>
        <nav aria-label="Primary destinations">
          {destinations.map((destination) => (
            <button
              aria-current={destination === activeDestination ? "page" : undefined}
              aria-label={destination}
              className={destination === activeDestination ? "active" : undefined}
              key={destination}
              onClick={() => setActiveDestination(destination)}
              type="button"
            >
              <span>{destination}</span>
              {destination === "To do" && <small aria-hidden="true">2</small>}
            </button>
          ))}
        </nav>
        <div className="member"><span>{initials}</span><div><strong>{session.organiserName}</strong><small>Household Organiser</small></div><button type="button" onClick={signOut}>Sign out</button></div>
      </aside>

      {activeDestination === "Options" ? <TrustedDevicesOptions
        accountService={accountService}
        session={session}
        trustedDeviceService={trustedDeviceService}
      /> : <main className="conversation">
        <header><div><small>Today</small><h1>New conversation</h1></div><span>Private conversation</span></header>
        <section className="messages" aria-label="Conversation">
          <article className="luna-message"><span aria-hidden="true">L</span><p>What would you like me to take care of?</p></article>
          {messages.map((message, index) => (
            <article className="member-message" key={`${message}-${index}`}><span aria-hidden="true">YC</span><p>{message}</p></article>
          ))}
        </section>
        <form className="composer" onSubmit={submitMessage}>
          <label htmlFor="message-composer">Message Luna</label>
          <textarea
            id="message-composer"
            onChange={(event) => setDraft(event.target.value)}
            placeholder="Message Luna or attach a document"
            rows={1}
            value={draft}
          />
          <button type="submit" aria-label="Send message">↑</button>
        </form>
      </main>}

      <aside className="context-panel">
        <header>Household context</header>
        <div><small>Household</small><strong>{session.householdName}</strong></div>
        <div><small>Desk status</small><strong>Ready</strong><p>Your cabinet will appear here after onboarding.</p></div>
        <div className="privacy"><small>Processing</small><strong>Local by default</strong><p>Luna will ask before using Cloud Assistance.</p></div>
      </aside>
    </div>
  );
}
