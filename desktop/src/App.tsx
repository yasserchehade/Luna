import { useCallback, useEffect, useState } from "react";
import "./App.css";
import type { AccountService, HouseholdSession } from "./account/accountService";
import { AccountFlow } from "./account/AccountFlow";
import { TrustedDeviceFlow, TrustedDeviceUnlock } from "./trusted-device/TrustedDeviceFlow";
import type { TrustedDeviceService } from "./trusted-device/trustedDeviceService";
import { synchronizeTrustedDevice } from "./trusted-device/trustedDeviceCoordinator";
import { TrustedDevicesOptions } from "./trusted-device/TrustedDevicesOptions";
import { CabinetSetup } from "./cabinet/CabinetSetup";
import type { CabinetService, CabinetValidation } from "./cabinet/cabinetService";
import { ConversationWorkspace } from "./conversation/ConversationWorkspace";
import type {
  AuditEvent,
  ConversationService,
  FiledOriginal,
} from "./conversation/conversationService";

const destinations = ["Luna", "To do", "Cabinet", "History", "Options"] as const;
type TrustedDeviceMode = "first" | "recovery";

type AppProps = {
  accountService: AccountService;
  cabinetService: CabinetService;
  conversationService: ConversationService;
  trustedDeviceService: TrustedDeviceService;
};

export default function App({ accountService, cabinetService, conversationService, trustedDeviceService }: AppProps) {
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
  const [activeDestination, setActiveDestination] = useState<(typeof destinations)[number]>("Luna");
  const [newConversationRequest, setNewConversationRequest] = useState(0);
  const [todoCount, setTodoCount] = useState(0);
  const [filedOriginals, setFiledOriginals] = useState<FiledOriginal[]>([]);
  const [auditEvents, setAuditEvents] = useState<AuditEvent[]>([]);
  const [documentSurfaceError, setDocumentSurfaceError] = useState("");

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

  useEffect(() => {
    if (!session || (activeDestination !== "Cabinet" && activeDestination !== "History")) return;
    const request = activeDestination === "Cabinet"
      ? conversationService.listFiledOriginals(session.householdId).then(setFiledOriginals)
      : conversationService.listAuditEvents(session.householdId).then(setAuditEvents);
    void request
      .then(() => setDocumentSurfaceError(""))
      .catch(() => setDocumentSurfaceError(`Luna could not open ${activeDestination}.`));
  }, [activeDestination, conversationService, session]);

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
        <button className="new-conversation" type="button" onClick={() => {
          setActiveDestination("Luna");
          setNewConversationRequest((request) => request + 1);
        }}>＋ New Conversation</button>
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
              {destination === "To do" && todoCount > 0 && <small aria-hidden="true">{todoCount}</small>}
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
        <header><div><small>Household cabinet</small><h1>Cabinet</h1></div><span>User-selected folder</span></header>
        <section className="cabinet-summary">
          <p><small>Location</small><strong>{cabinetValidation.configuration.root}</strong></p>
          <div>
            <article><span aria-hidden="true">▰</span><strong>Incoming</strong></article>
            {cabinetValidation.configuration.sections.map((section) => <article key={section}><span aria-hidden="true">▰</span><strong>{section}</strong></article>)}
          </div>
          {documentSurfaceError && <p role="alert">{documentSurfaceError}</p>}
          {filedOriginals.length === 0
            ? <p className="empty-state">No Originals have been filed yet.</p>
            : <div className="filed-originals">{filedOriginals.map((filedOriginal) => <article key={filedOriginal.arrivalId}>
              <strong>{filedOriginal.filingDecision.fileName}</strong>
              <p>{filedOriginal.filingDecision.cabinetDestination}</p>
              <small>SHA-256 {filedOriginal.checksum}</small>
            </article>)}</div>}
        </section>
      </main> : activeDestination === "History" ? <main className="conversation">
        <header><div><small>Household history</small><h1>History</h1></div></header>
        <section className="messages">
          {documentSurfaceError && <p role="alert">{documentSurfaceError}</p>}
          {auditEvents.length === 0
            ? <p className="empty-state">No consequential document actions yet.</p>
            : auditEvents.map((event) => <article className="history-event" key={event.id}>
              <strong>Document filed</strong>
              <p>{event.subject}</p>
              <small>{event.outcome} · Verified SHA-256 {event.filedOriginal.checksum}</small>
            </article>)}
        </section>
      </main> : <>
        {coordinationNotice && <p role="status" className="session-notice">{coordinationNotice}</p>}
        <ConversationWorkspace
          conversationService={conversationService}
          destination={activeDestination}
          householdId={session.householdId}
          newConversationRequest={newConversationRequest}
          onOpenConversation={() => setActiveDestination("Luna")}
          onTodoCountChange={setTodoCount}
        />
      </>}

      <aside className="context-panel">
        <header>Household context</header>
        <div><small>Household</small><strong>{session.householdName}</strong></div>
        <div><small>Desk status</small><strong>Ready</strong><p>{cabinetValidation.configuration.root}</p></div>
        <div className="privacy"><small>Processing</small><strong>Local by default</strong><p>Luna will ask before using Cloud Assistance.</p></div>
      </aside>
    </div>
  );
}
