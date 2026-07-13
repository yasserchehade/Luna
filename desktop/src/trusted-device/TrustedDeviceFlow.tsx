import { FormEvent, useState } from "react";
import type { AccountService, HouseholdSession } from "../account/accountService";
import type { TrustedDeviceEnrollment, TrustedDeviceService } from "./trustedDeviceService";

type TrustedDeviceFlowProps = {
  accountService: AccountService;
  mode: "first" | "recovery";
  session: HouseholdSession;
  trustedDeviceService: TrustedDeviceService;
  onTrusted: () => void;
};

type Step =
  | { kind: "intro" }
  | { kind: "authenticator"; factorId: string; qrCode: string; secret: string }
  | ({ kind: "saveRecovery" } & TrustedDeviceEnrollment)
  | { kind: "enterRecovery"; recoveryEnvelope: string; keyEpoch: number }
  | { kind: "devicePin" };

export function TrustedDeviceFlow({
  accountService,
  mode,
  session,
  trustedDeviceService,
  onTrusted,
}: TrustedDeviceFlowProps) {
  const [step, setStep] = useState<Step>({ kind: "intro" });
  const [error, setError] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);

  const run = async (action: () => Promise<void>, failureMessage: string) => {
    if (isSubmitting) return;
    setIsSubmitting(true);
    setError("");
    try {
      await action();
    } catch {
      setError(failureMessage);
    } finally {
      setIsSubmitting(false);
    }
  };

  const beginAuthenticator = () => run(async () => {
    const enrollment = await accountService.beginAuthenticatorEnrollment();
    setStep({ kind: "authenticator", ...enrollment });
  }, "We could not start authenticator setup. Check your connection and try again.");

  const verifyAuthenticator = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (step.kind !== "authenticator") return;
    const form = new FormData(event.currentTarget);
    void run(async () => {
      await accountService.verifyAuthenticatorEnrollment(step.factorId, String(form.get("code")));
      const enrollment = await trustedDeviceService.enrolFirstDevice(session.householdId);
      const registered = await accountService.registerFirstTrustedDevice({
        label: "This device",
        publicKey: enrollment.devicePublicKey,
        keyEnvelope: enrollment.deviceKeyEnvelope,
        recoveryEnvelope: enrollment.recoveryEnvelope,
        recoveryVerificationKey: enrollment.recoveryVerificationKey,
      });
      await trustedDeviceService.setCurrentKeyEpoch(session.householdId, registered.keyEpoch);
      setStep({ kind: "saveRecovery", ...enrollment });
    }, "That authenticator code did not work. Check the current code and try again.");
  };

  const confirmRecoveryKey = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (step.kind !== "saveRecovery") return;
    const form = new FormData(event.currentTarget);
    void run(async () => {
      await trustedDeviceService.confirmRecoveryKey(
        session.householdId,
        String(form.get("recoveryKey")),
        step.recoveryEnvelope,
      );
      setStep({ kind: "devicePin" });
    }, "That Recovery Key does not match. Check every word and try again.");
  };

  const beginRecovery = () => run(async () => {
    const recovery = await accountService.getTrustedDeviceRecoveryEnvelope();
    setStep({ kind: "enterRecovery", ...recovery });
  }, "We could not start Trusted Device recovery. Check your connection and try again.");

  const recoverDevice = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (step.kind !== "enterRecovery") return;
    const form = new FormData(event.currentTarget);
    void run(async () => {
      const recovered = await trustedDeviceService.recoverDevice(
        session.householdId,
        String(form.get("recoveryKey")),
        step.recoveryEnvelope,
        step.keyEpoch,
      );
      const registered = await accountService.registerRecoveredTrustedDevice({
        label: "Recovered device",
        publicKey: recovered.devicePublicKey,
        keyEnvelope: recovered.deviceKeyEnvelope,
        keyEpoch: step.keyEpoch,
        recoveryAuthorizationSignature: recovered.recoveryAuthorizationSignature,
      });
      await trustedDeviceService.finalizeRecoveredDevice(session.householdId, registered.keyEpoch);
      setStep({ kind: "devicePin" });
    }, "That Recovery Key does not match. Check every word and try again.");
  };

  const configureDevicePin = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const form = new FormData(event.currentTarget);
    const pin = String(form.get("pin"));
    if (pin !== String(form.get("pinConfirmation"))) {
      setError("The PINs do not match.");
      return;
    }
    void run(async () => {
      await trustedDeviceService.configureDevicePin(session.householdId, pin);
      onTrusted();
    }, "Use a PIN containing at least six digits, then try again.");
  };

  if (step.kind === "intro") {
    if (mode === "recovery") {
      return <TrustCard title="Recover this trusted device" description="Use your Recovery Key to let this device open your existing Household memory.">
        <button type="button" disabled={isSubmitting} onClick={beginRecovery}>Use Recovery Key</button>
        <TrustError message={error} />
      </TrustCard>;
    }
    return <TrustCard title="Protect this trusted device" description="Your authenticator and this device's protected keys keep Household memory separate from your Luna account password.">
      <button type="button" disabled={isSubmitting} onClick={beginAuthenticator}>Set up authenticator</button>
      <TrustError message={error} />
    </TrustCard>;
  }

  if (step.kind === "authenticator") {
    return <TrustCard title="Connect your authenticator" description="Use Google Authenticator, Microsoft Authenticator, or another TOTP app. Scan the code, or enter the setup key manually, then type the current six-digit code.">
      <img className="authenticator-qr" src={step.qrCode} alt="Scan this QR code with your authenticator app" />
      <p className="secret-label">Manual setup key</p>
      <code id="authenticator-secret">{step.secret}</code>
      <form onSubmit={verifyAuthenticator}>
        <label htmlFor="authenticator-code">Authenticator code</label>
        <input id="authenticator-code" name="code" inputMode="numeric" autoComplete="one-time-code" required />
        <TrustError message={error} />
        <button type="submit" disabled={isSubmitting}>Verify authenticator</button>
      </form>
    </TrustCard>;
  }

  if (step.kind === "enterRecovery") {
    return <TrustCard title="Enter your Recovery Key" description="Luna uses this key locally to recover Household memory for this device.">
      <form onSubmit={recoverDevice}>
        <label htmlFor="replacement-recovery-key">Recovery Key</label>
        <textarea id="replacement-recovery-key" name="recoveryKey" rows={4} required />
        <TrustError message={error} />
        <button type="submit" disabled={isSubmitting}>Recover trusted device</button>
      </form>
    </TrustCard>;
  }

  if (step.kind === "devicePin") {
    return <TrustCard title="Create a device PIN" description="Use this PIN to unlock Household memory on this device. It is separate from your Luna account password.">
      <form onSubmit={configureDevicePin}>
        <label htmlFor="device-pin">Device PIN</label>
        <input id="device-pin" name="pin" type="password" inputMode="numeric" minLength={6} required />
        <label htmlFor="device-pin-confirmation">Confirm device PIN</label>
        <input id="device-pin-confirmation" name="pinConfirmation" type="password" inputMode="numeric" minLength={6} required />
        <TrustError message={error} />
        <button type="submit" disabled={isSubmitting}>Save device PIN</button>
      </form>
    </TrustCard>;
  }

  return <TrustCard title="Save your Recovery Key" description="Keep this key offline. It is the only way to recover Household memory without an existing trusted device.">
    <output id="recovery-key">{step.recoveryKey}</output>
    <form onSubmit={confirmRecoveryKey}>
      <label htmlFor="recovery-key-confirmation">Re-enter your Recovery Key to confirm you saved it</label>
      <textarea id="recovery-key-confirmation" name="recoveryKey" rows={4} required />
      <TrustError message={error} />
      <button type="submit" disabled={isSubmitting}>Confirm Recovery Key</button>
    </form>
  </TrustCard>;
}

function TrustCard({ title, description, children }: React.PropsWithChildren<{ title: string; description: string }>) {
  return <main className="account-screen"><section className="account-card trust-card">
    <div className="account-brand"><span aria-hidden="true">L</span><strong>Luna</strong></div>
    <p className="eyebrow">Trusted Device setup</p>
    <h1>{title}</h1>
    <p>{description}</p>
    {children}
  </section></main>;
}

function TrustError({ message }: { message: string }) {
  return message ? <p className="account-error" role="alert">{message}</p> : null;
}

export function TrustedDeviceUnlock({
  session,
  trustedDeviceService,
  onUnlocked,
}: {
  session: HouseholdSession;
  trustedDeviceService: TrustedDeviceService;
  onUnlocked: () => Promise<void> | void;
}) {
  const [error, setError] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);
  const unlock = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (isSubmitting) return;
    const form = new FormData(event.currentTarget);
    setIsSubmitting(true);
    setError("");
    void trustedDeviceService.unlockDevice(session.householdId, String(form.get("pin")))
      .then(() => onUnlocked())
      .catch(() => setError("That device PIN did not work. Try again."))
      .finally(() => setIsSubmitting(false));
  };
  return <TrustCard title="Unlock this trusted device" description="Enter this device's local PIN before Luna opens Household memory.">
    <form onSubmit={unlock}>
      <label htmlFor="device-unlock-pin">Device PIN</label>
      <input id="device-unlock-pin" name="pin" type="password" inputMode="numeric" required autoFocus />
      <TrustError message={error} />
      <button type="submit" disabled={isSubmitting}>Unlock Luna</button>
    </form>
  </TrustCard>;
}
