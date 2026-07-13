import { FormEvent, useEffect, useState } from "react";
import type { AccountService, HouseholdSession, TrustedDeviceRecord } from "../account/accountService";
import type { TrustedDeviceService } from "./trustedDeviceService";

export function TrustedDevicesOptions({
  accountService,
  session,
  trustedDeviceService,
}: {
  accountService: AccountService;
  session: HouseholdSession;
  trustedDeviceService: TrustedDeviceService;
}) {
  const [devices, setDevices] = useState<TrustedDeviceRecord[]>([]);
  const [currentPublicKey, setCurrentPublicKey] = useState("");
  const [target, setTarget] = useState<TrustedDeviceRecord | null>(null);
  const [error, setError] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);

  useEffect(() => {
    void Promise.all([
      accountService.listTrustedDevices(),
      trustedDeviceService.currentDevicePublicKey(session.householdId),
    ])
      .then(([trustedDevices, publicKey]) => {
        setDevices(trustedDevices);
        setCurrentPublicKey(publicKey);
      })
      .catch(() => setError("Luna could not load Trusted Devices. Try again."));
  }, [accountService, session.householdId, trustedDeviceService]);

  const revoke = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!target || isSubmitting) return;
    const form = new FormData(event.currentTarget);
    setIsSubmitting(true);
    setError("");
    let serviceRotationCommitted = false;
    void (async () => {
      const recovery = await accountService.getTrustedDeviceRecoveryEnvelope();
      const retainedPublicKeys = devices
        .filter(({ id, status }) => id !== target.id && status === "active")
        .map(({ publicKey }) => publicKey);
      const rotation = await trustedDeviceService.prepareHouseholdKeyRotation(
        session.householdId,
        String(form.get("recoveryKey")),
        recovery.recoveryEnvelope,
        retainedPublicKeys,
        recovery.keyEpoch,
        target.id,
      );
      try {
        const updated = await accountService.revokeTrustedDevice({
          deviceId: target.id,
          currentDevicePublicKey: currentPublicKey,
          currentKeyEpoch: recovery.keyEpoch,
          recoveryEnvelope: rotation.recoveryEnvelope,
          deviceEnvelopes: rotation.deviceEnvelopes,
          recoveryAuthorizationSignature: rotation.recoveryAuthorizationSignature,
        });
        serviceRotationCommitted = true;
        await trustedDeviceService.finalizeHouseholdKeyRotation(
          session.householdId,
          recovery.keyEpoch + 1,
        );
        setDevices(updated);
        setTarget(null);
      } catch (cause) {
        if (!serviceRotationCommitted) {
          await trustedDeviceService.discardHouseholdKeyRotation(session.householdId);
        }
        throw cause;
      }
    })()
      .catch(() => setError("Luna could not revoke that device. Check the Recovery Key and try again."))
      .finally(() => setIsSubmitting(false));
  };

  return <main className="conversation options-view">
    <header><div><small>Options</small><h1>Trusted devices</h1></div><span>Household security</span></header>
    <section className="device-settings">
      <p>Only active Trusted Devices can open Household memory. Revocation rotates the Household key for every device that remains active.</p>
      {error && <p className="account-error" role="alert">{error}</p>}
      <ul>
        {devices.map((device) => {
          const isCurrent = device.publicKey === currentPublicKey;
          return <li key={device.id} data-device-label={device.label}>
            <div><strong>{device.label}</strong><span>{isCurrent ? "This device" : device.status === "active" ? "Active" : "Revoked"}</span></div>
            {device.status === "active" && !isCurrent
              ? <button type="button" aria-label={`Revoke ${device.label}`} onClick={() => setTarget(device)}>Revoke</button>
              : null}
          </li>;
        })}
      </ul>
      {target && <section className="revocation-card">
        <h2>Confirm device revocation</h2>
        <p>Enter your offline Recovery Key. Luna will rotate Household memory keys before future state is written.</p>
        <form onSubmit={revoke}>
          <label htmlFor="revocation-recovery-key">Recovery Key</label>
          <textarea id="revocation-recovery-key" name="recoveryKey" rows={4} required />
          <button type="submit" disabled={isSubmitting}>Revoke device</button>
          <button type="button" disabled={isSubmitting} onClick={() => setTarget(null)}>Cancel</button>
        </form>
      </section>}
    </section>
  </main>;
}
