import { FormEvent, useState } from "react";
import type { AccountService, HouseholdSession } from "../account/accountService";
import type { RecoveryKeyReplacement, TrustedDeviceService } from "./trustedDeviceService";

type ReplacementDraft = RecoveryKeyReplacement & {
  commitState: "ready" | "uncertain";
  currentRecoveryVerificationKey: string;
  keyEpoch: number;
};

type RecoveryReplacementStep =
  | { kind: "idle" }
  | { kind: "verify" }
  | ({ kind: "save" } & ReplacementDraft)
  | { kind: "complete" };

export function RecoveryKeyReplacementOptions({
  accountService,
  currentDevicePublicKey,
  session,
  trustedDeviceService,
}: {
  accountService: AccountService;
  currentDevicePublicKey: string;
  session: HouseholdSession;
  trustedDeviceService: TrustedDeviceService;
}) {
  const [step, setStep] = useState<RecoveryReplacementStep>({ kind: "idle" });
  const [error, setError] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);

  const verify = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (step.kind !== "verify" || isSubmitting) return;
    const form = new FormData(event.currentTarget);
    setIsSubmitting(true);
    setError("");
    void (async () => {
      await accountService.verifyAuthenticatorChallenge(String(form.get("authenticatorCode")));
      const recovery = await accountService.getTrustedDeviceRecoveryEnvelope();
      const replacement = await trustedDeviceService.prepareRecoveryKeyReplacement(
        session.householdId,
        recovery.keyEpoch,
        recovery.recoveryVerificationKey,
      );
      setStep({
        kind: "save",
        commitState: "ready",
        currentRecoveryVerificationKey: recovery.recoveryVerificationKey,
        keyEpoch: recovery.keyEpoch,
        ...replacement,
      });
    })()
      .catch((cause: unknown) => {
        const detail = String(cause);
        setError(detail.includes("must be re-enrolled")
          ? "This device was enrolled by an earlier beta and must be re-enrolled before it can replace a Recovery Key."
          : "Luna could not verify this Recovery Key Replacement. Check the authenticator code and try again.");
      })
      .finally(() => setIsSubmitting(false));
  };

  const reconcile = async (draft: ReplacementDraft) => {
    try {
      const current = await accountService.getTrustedDeviceRecoveryEnvelope();
      if (
        current.keyEpoch === draft.keyEpoch
        && current.recoveryEnvelope === draft.recoveryEnvelope
        && current.recoveryVerificationKey === draft.recoveryVerificationKey
      ) {
        setStep({ kind: "complete" });
        setError("");
        return;
      }
      if (
        current.keyEpoch === draft.keyEpoch
        && current.recoveryVerificationKey === draft.currentRecoveryVerificationKey
      ) {
        setStep({ kind: "save", ...draft, commitState: "ready" });
        setError("The replacement was not committed. Your previous Recovery Key still works; retry, or cancel and begin again for fresh authenticator verification.");
        return;
      }
      setStep({ kind: "idle" });
      setError("Recovery authority changed elsewhere. This displayed key is not active; begin again from the current Trusted Device state.");
    } catch {
      setStep({ kind: "save", ...draft, commitState: "uncertain" });
      setError("Luna could not confirm whether the replacement committed. Keep this key visible and check replacement status when connected.");
    }
  };

  const confirm = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (step.kind !== "save" || step.commitState !== "ready" || isSubmitting) return;
    const draft = step;
    const form = new FormData(event.currentTarget);
    setIsSubmitting(true);
    setError("");
    void (async () => {
      try {
        await trustedDeviceService.confirmRecoveryKeyReplacement(
          session.householdId,
          String(form.get("recoveryKey")),
          draft.recoveryEnvelope,
        );
      } catch {
        setError("That replacement Recovery Key does not match. Check every word and try again.");
        return;
      }

      try {
        await accountService.replaceRecoveryKey({
          currentDevicePublicKey,
          currentKeyEpoch: draft.keyEpoch,
          currentRecoveryVerificationKey: draft.currentRecoveryVerificationKey,
          recoveryEnvelope: draft.recoveryEnvelope,
          recoveryVerificationKey: draft.recoveryVerificationKey,
          deviceAuthorizationSignature: draft.deviceAuthorizationSignature,
        });
        setStep({ kind: "complete" });
      } catch {
        await reconcile(draft);
      }
    })().finally(() => setIsSubmitting(false));
  };

  const checkStatus = () => {
    if (step.kind !== "save" || step.commitState !== "uncertain" || isSubmitting) return;
    setIsSubmitting(true);
    setError("");
    void reconcile(step).finally(() => setIsSubmitting(false));
  };

  return <section className="account-session-card">
    <h2>Recovery Key</h2>
    <p>If the offline Recovery Key is lost, this unlocked Trusted Device can replace it after authenticator verification.</p>
    {error && <p className="account-error" role="alert">{error}</p>}
    {step.kind === "idle" && <button
      type="button"
      onClick={() => {
        setError("");
        setStep({ kind: "verify" });
      }}
    >Replace lost Recovery Key</button>}
    {step.kind === "verify" && <section className="revocation-card">
      <h2>Verify Recovery Key Replacement</h2>
      <p>Enter a fresh code from your authenticator app. Account access alone cannot replace cabinet recovery authority.</p>
      <form onSubmit={verify}>
        <label htmlFor="replacement-authenticator-code">Authenticator code</label>
        <input id="replacement-authenticator-code" name="authenticatorCode" inputMode="numeric" autoComplete="one-time-code" required />
        <button type="submit" disabled={isSubmitting}>Verify and generate replacement</button>
        <button type="button" disabled={isSubmitting} onClick={() => setStep({ kind: "idle" })}>Cancel</button>
      </form>
    </section>}
    {step.kind === "save" && <section className="revocation-card">
      <h2>Save your replacement Recovery Key</h2>
      <p>This key is shown once. Store it offline, then re-enter all 24 words. Nothing changes until confirmation succeeds.</p>
      <output id="replacement-recovery-key-output">{step.recoveryKey}</output>
      {step.commitState === "ready" ? <form onSubmit={confirm}>
        <label htmlFor="replacement-recovery-key-confirmation">Re-enter your replacement Recovery Key</label>
        <textarea id="replacement-recovery-key-confirmation" name="recoveryKey" rows={4} required />
        <button type="submit" disabled={isSubmitting}>Confirm replacement Recovery Key</button>
        <button type="button" disabled={isSubmitting} onClick={() => setStep({ kind: "idle" })}>Cancel</button>
      </form> : <button type="button" disabled={isSubmitting} onClick={checkStatus}>Check replacement status</button>}
    </section>}
    {step.kind === "complete" && <p role="status">Recovery Key replaced. The previous Recovery Key no longer works.</p>}
  </section>;
}
