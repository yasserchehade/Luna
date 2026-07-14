import { FormEvent, useCallback, useEffect, useState } from "react";
import "./App.css";
import type { AccountService, HouseholdSession } from "./account/accountService";
import { AccountFlow } from "./account/AccountFlow";
import { TrustedDeviceFlow, TrustedDeviceUnlock } from "./trusted-device/TrustedDeviceFlow";
import type { TrustedDeviceService } from "./trusted-device/trustedDeviceService";
import { synchronizeTrustedDevice } from "./trusted-device/trustedDeviceCoordinator";
import { TrustedDevicesOptions } from "./trusted-device/TrustedDevicesOptions";
import { CabinetSetup } from "./cabinet/CabinetSetup";
import type { CabinetService, CabinetValidation } from "./cabinet/cabinetService";

const destinations = ["Luna", "To do", "Cabinet", "History", "Options"] as const;
type TrustedDeviceMode = "first" | "recovery";

type AppProps = {
  accountService: AccountService;
  cabinetService: CabinetService;
  trustedDeviceService: TrustedDeviceService;
};

export default function App({ accountService, cabinetService, trustedDeviceService }: AppProps) {
  const [isRestoringSession, setIsRestoringSession] = useState(true);
  const [restoreAttempt, setRestoreAttempt] = useState(0);
  const [restoreFailed, setRestoreFailed] = useState(false);
  const [signOutFailed, setSignOutFailed] = useState(false);
  const [coordinationNotice, setCoordinationNotice] = useState("");
  const [session, setSession] = useState<HouseholdSession | null>(null);
  const [cabinetValidation, setCabinetValidation] = useState<CabinetValidation | null>();
  const [cabinetCheckFailed, setCabinetCheckFailed] = useState(false);
  const [cabinetCheckAttempt, setCabinetCheckAttempt] = useState(0);
  const [pendingTrustedSession, setPendingTrustedSession] = useState<{
    session: HouseholdSession;
    mode: TrustedDeviceMode;
  } | null>(null);
  const [pendingUnlockSession, setPendingUnlockSession] = useState<{
    session: HouseholdSession;
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

  const lockLuna = async () => {
    if (!session) return;
    await trustedDeviceService.lockDevice(session.householdId);
    setPendingUnlockSession({ session });
    setSession(null);
  };

  const signOut = useCallback(async (protectedSession?: HouseholdSession) => {
    if (protectedSession) await trustedDeviceService.lockDevice(protectedSession.householdId);
    setAccountEntry("signIn");
    setSession(null);
    setCabinetValidation(undefined);
    setCabinetCheckFailed(false);
    setPendingUnlockSession(null);
    setPendingTrustedSession(null);
    setCoordinationNotice("");
    try {
      await accountService.signOut();
      setSignOutFailed(false);
    } catch {
      setSignOutFailed(true);
    }
  }, [accountService, trustedDeviceService]);

  const synchronizeAfterUnlock = useCallback(async (unlockedSession: HouseholdSession) => {
    try {
      if (await synchronizeTrustedDevice(accountService, trustedDeviceService, unlockedSession) === "revoked") {
        await trustedDeviceService.forgetCurrentDevice(unlockedSession.householdId);
        await signOut(unlockedSession);
        return;
      }
      setCoordinationNotice("");
    } catch {
      setCoordinationNotice(
        "Luna is working offline. Trusted Device changes will be checked when the connection returns.",
      );
    }
  }, [accountService, signOut, trustedDeviceService]);

  const authenticate = useCallback(async (
    authenticatedSession: HouseholdSession,
    source: "interactive" | "restored" = "interactive",
  ) => {
    if (await trustedDeviceService.isCurrentDeviceTrusted(authenticatedSession.householdId)) {
      if (
        source === "interactive"
        && await synchronizeTrustedDevice(accountService, trustedDeviceService, authenticatedSession) === "revoked"
      ) {
        await trustedDeviceService.forgetCurrentDevice(authenticatedSession.householdId);
        setPendingTrustedSession({ session: authenticatedSession, mode: "recovery" });
        return;
      }
      const isUnlocked = await trustedDeviceService.isCurrentDeviceUnlocked(authenticatedSession.householdId);
      if (isUnlocked) {
        setSession(authenticatedSession);
        if (source === "restored") void synchronizeAfterUnlock(authenticatedSession);
      } else {
        setPendingUnlockSession({ session: authenticatedSession });
      }
      return;
    }
    if (source === "restored") {
      await signOut(authenticatedSession);
      return;
    }
    const authenticatorStatus = await accountService.getAuthenticatorStatus();
    setPendingTrustedSession({
      session: authenticatedSession,
      mode: authenticatorStatus === "unenrolled" ? "first" : "recovery",
    });
  }, [accountService, signOut, synchronizeAfterUnlock, trustedDeviceService]);

  useEffect(() => {
    let active = true;
    void accountService.restoreSession()
      .then(async (restoredSession) => {
        if (active && restoredSession) await authenticate(restoredSession, "restored");
      })
      .catch(() => {
        if (active) setRestoreFailed(true);
      })
      .finally(() => {
        if (active) setIsRestoringSession(false);
      });
    return () => {
      active = false;
    };
  }, [accountService, authenticate, restoreAttempt]);

  useEffect(() => {
    if (!session || !coordinationNotice) return;
    const retryCoordination = () => void synchronizeAfterUnlock(session);
    const retryTimer = window.setInterval(retryCoordination, 30_000);
    window.addEventListener("online", retryCoordination);
    return () => {
      window.clearInterval(retryTimer);
      window.removeEventListener("online", retryCoordination);
    };
  }, [coordinationNotice, session, synchronizeAfterUnlock]);

  useEffect(() => {
    if (!session) {
      setCabinetValidation(undefined);
      return;
    }
    let active = true;
    setCabinetValidation(undefined);
    setCabinetCheckFailed(false);
    void cabinetService.validate(session.householdId)
      .then((validation) => {
        if (active) setCabinetValidation(validation);
      })
      .catch(() => {
        if (active) setCabinetCheckFailed(true);
      });
    return () => {
      active = false;
    };
  }, [cabinetCheckAttempt, cabinetService, session?.householdId]);

  if (isRestoringSession) {
    return <main className="account-screen"><section className="account-card">
      <div className="account-brand"><span aria-hidden="true">L</span><strong>Luna</strong></div>
      <p className="eyebrow">Trusted device</p>
      <h1>Opening Luna</h1>
      <p>Checking this device's protected account session.</p>
    </section></main>;
  }

  if (restoreFailed) {
    return <main className="account-screen"><section className="account-card">
      <div className="account-brand"><span aria-hidden="true">L</span><strong>Luna</strong></div>
      <p className="eyebrow">Trusted device</p>
      <h1>Luna could not check this session</h1>
      <p>Retry the protected session check, or sign in again after the connection or credential vault is available.</p>
      <button type="button" onClick={() => {
        setRestoreFailed(false);
        setIsRestoringSession(true);
        setRestoreAttempt((attempt) => attempt + 1);
      }}>Retry session check</button>
    </section></main>;
  }

  if (signOutFailed) {
    return <main className="account-screen"><section className="account-card">
      <div className="account-brand"><span aria-hidden="true">L</span><strong>Luna</strong></div>
      <p className="eyebrow">Account security</p>
      <h1>Finish signing out</h1>
      <p>Luna is locked and no Household information is visible, but the protected account session could not be removed from this device.</p>
      <button type="button" onClick={() => void signOut()}>Retry sign out</button>
    </section></main>;
  }

  if (pendingUnlockSession) {
    return <TrustedDeviceUnlock
      session={pendingUnlockSession.session}
      trustedDeviceService={trustedDeviceService}
      onSignOut={() => signOut(pendingUnlockSession.session)}
      onUnlocked={() => {
        const unlockingSession = pendingUnlockSession.session;
        setSession(unlockingSession);
        setPendingUnlockSession(null);
        void synchronizeAfterUnlock(unlockingSession);
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

  if (cabinetCheckFailed) {
    return <main className="account-screen"><section className="account-card">
      <div className="account-brand"><span aria-hidden="true">L</span><strong>Luna</strong></div>
      <p className="eyebrow">Desk check</p>
      <h1>Luna could not check the cabinet</h1>
      <p>The remembered cabinet was not changed. Check the device and try again.</p>
      <button type="button" onClick={() => setCabinetCheckAttempt((attempt) => attempt + 1)}>Check again</button>
    </section></main>;
  }

  if (cabinetValidation === undefined) {
    return <main className="account-screen"><section className="account-card">
      <div className="account-brand"><span aria-hidden="true">L</span><strong>Luna</strong></div>
      <p className="eyebrow">Desk check</p>
      <h1>Checking Luna's desk</h1>
      <p>Validating this Household's remembered cabinet.</p>
    </section></main>;
  }

  if (!cabinetValidation || cabinetValidation.availability === "unavailable") {
    return <CabinetSetup
      cabinetService={cabinetService}
      onConfigured={(configuration) => setCabinetValidation({ configuration, availability: "ready" })}
      session={session}
      unavailableRoot={cabinetValidation?.configuration.root}
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
        <div className="member"><span>{initials}</span><div><strong>{session.organiserName}</strong><small>Household Organiser</small></div><button type="button" onClick={lockLuna}>Lock Luna</button></div>
      </aside>

      {activeDestination === "Options" ? <TrustedDevicesOptions
        accountService={accountService}
        onSignOut={() => signOut(session)}
        session={session}
        trustedDeviceService={trustedDeviceService}
      /> : activeDestination === "Cabinet" ? <main className="conversation cabinet-view">
        <header><div><small>Household cabinet</small><h1>Cabinet</h1></div><span>{cabinetValidation.configuration.storage === "cloudSynchronized" ? "Cloud-synchronised" : "On this device"}</span></header>
        <section className="cabinet-summary">
          <p><small>Location</small><strong>{cabinetValidation.configuration.root}</strong></p>
          <div>{cabinetValidation.configuration.sections.map((section) => <article key={section}><span aria-hidden="true">▰</span><strong>{section}</strong></article>)}</div>
        </section>
      </main> : <main className="conversation">
        <header><div><small>Today</small><h1>New conversation</h1></div><span>Private conversation</span></header>
        {coordinationNotice && <p role="status" className="session-notice">{coordinationNotice}</p>}
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
        <div><small>Desk status</small><strong>Ready</strong><p>{cabinetValidation.configuration.root}</p></div>
        <div className="privacy"><small>Processing</small><strong>Local by default</strong><p>Luna will ask before using Cloud Assistance.</p></div>
      </aside>
    </div>
  );
}
