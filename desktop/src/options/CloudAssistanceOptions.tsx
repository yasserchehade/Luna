import { useEffect, useState } from "react";
import type {
  CloudConsentScope,
  ConversationService,
  IntelligenceProviderStatus,
} from "../conversation/conversationService";

export function CloudAssistanceOptions({ conversationService, householdId }: { conversationService: ConversationService; householdId: string }) {
  const [providers, setProviders] = useState<IntelligenceProviderStatus[]>([]);
  const [scopes, setScopes] = useState<CloudConsentScope[]>([]);
  const [credentialDrafts, setCredentialDrafts] = useState<Record<string, string>>({});
  const [busyProviderId, setBusyProviderId] = useState("");
  const [error, setError] = useState("");

  const refresh = async () => {
    try {
      const [providerStatuses, consentScopes] = await Promise.all([
        conversationService.listIntelligenceProviderStatuses(householdId),
        conversationService.listCloudConsentScopes(householdId),
      ]);
      setProviders(providerStatuses);
      setScopes(consentScopes);
      setError("");
    } catch (reason) {
      setError(String(reason));
    }
  };

  useEffect(() => { void refresh(); }, [conversationService, householdId]);

  const revoke = async (scopeId: number) => {
    try {
      await conversationService.revokeCloudConsentScope(householdId, scopeId);
      await refresh();
    } catch (reason) {
      setError(String(reason));
    }
  };

  const testAndSaveProvider = async (providerId: string) => {
    const credential = credentialDrafts[providerId]?.trim();
    if (!credential) return;
    setBusyProviderId(providerId);
    try {
      await conversationService.testAndSetIntelligenceProviderCredential(
        householdId,
        providerId,
        credential,
      );
      setCredentialDrafts((current) => ({ ...current, [providerId]: "" }));
      await refresh();
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusyProviderId("");
    }
  };

  const removeProvider = async (providerId: string) => {
    setBusyProviderId(providerId);
    try {
      await conversationService.clearIntelligenceProviderCredential(householdId, providerId);
      setCredentialDrafts((current) => ({ ...current, [providerId]: "" }));
      await refresh();
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusyProviderId("");
    }
  };

  return <section className="cloud-assistance-options" aria-label="Cloud assistance">
    <h2>Cloud assistance</h2>
    <p className="muted">Luna stays local by default. An eligible paid Household receives Luna-managed Intelligence automatically. A free or paid Household can connect its own supported provider here, or remain local-only.</p>
    <p className="muted">Managed gateway credentials are provisioned to Trusted Devices by Luna and never entered by a Household Member. Upstream provider credentials remain on Luna&apos;s managed gateway and never reach this desktop.</p>
    <div className="cloud-provider-list">
      {providers.filter(({ descriptor }) => descriptor.managedByLuna).map(({ descriptor, configured }) => <article className="cloud-provider-card" key={descriptor.id}>
        <div className="cloud-provider-heading">
          <div><strong>{descriptor.name}</strong><span>{descriptor.description}</span></div>
          <small>{configured ? "Managed access ready" : "Managed access not provisioned"}</small>
        </div>
        <p>Approved models: {descriptor.models.map(({ name }) => name).join(", ")}</p>
        {!configured && <p className="muted">There is nothing to paste here. Luna will enable this connection automatically when the Household plan and Trusted Device are eligible.</p>}
      </article>)}
    </div>
    <h3>Bring your own provider</h3>
    <p className="muted">The provider bills your account. Luna tests the key through a BYOK-only route before saving it in this device&apos;s operating-system credential vault. Luna never falls back to a managed route.</p>
    <div className="cloud-provider-list">
      {providers.filter(({ descriptor }) => !descriptor.managedByLuna).map(({ descriptor, configured }) => <section
        className="cloud-provider-card"
        aria-label={`${descriptor.name.split(" — ")[0]} bring-your-own-key connection`}
        key={descriptor.id}
      >
        <div className="cloud-provider-heading">
          <div><strong>{descriptor.name}</strong><span>{descriptor.description}</span></div>
          <small>{configured ? "Connected" : "Not connected"}</small>
        </div>
        <p>Approved models: {descriptor.models.map(({ name }) => name).join(", ")}</p>
        {descriptor.authUrl && <a href={descriptor.authUrl} target="_blank" rel="noreferrer">Open {descriptor.name.split(" — ")[0]} API keys</a>}
        <label>
          {configured ? "Replacement provider API key" : "Provider API key"}
          <input
            type="password"
            autoComplete="off"
            value={credentialDrafts[descriptor.id] ?? ""}
            onChange={(event) => setCredentialDrafts((current) => ({
              ...current,
              [descriptor.id]: event.target.value,
            }))}
          />
        </label>
        <div className="default-intelligence-actions">
          <button
            type="button"
            disabled={busyProviderId === descriptor.id || !credentialDrafts[descriptor.id]?.trim()}
            onClick={() => void testAndSaveProvider(descriptor.id)}
          >{configured ? "Test and replace" : "Test and connect"}</button>
          {configured && <button
            type="button"
            disabled={busyProviderId === descriptor.id}
            onClick={() => void removeProvider(descriptor.id)}
          >Remove key</button>}
        </div>
      </section>)}
    </div>
    <h3>Consent Grants</h3>
    {scopes.length === 0
      ? <p className="muted">No Consent Grant has been recorded.</p>
      : <ul className="consent-scope-list">{scopes.map((scope) => <li key={scope.id} className={scope.revoked ? "revoked" : undefined}>
        <div>
          <strong>{scope.providerId} · {scope.modelId}</strong>
          <span>{scope.purpose} · {scope.kind === "oneTime" ? `Document ${scope.documentArrivalId ?? ""}` : scope.futureScope}</span>
          <span>Disclosed fields: {scope.fields.join(", ")}</span>
          <small>{scope.revoked ? "Revoked" : scope.consumedAt ? "Used" : "Active"} · granted by {scope.grantedBy} · {scope.createdAt}</small>
        </div>
        {!scope.revoked && scope.kind === "reusable" && <button type="button" onClick={() => void revoke(scope.id)}>Revoke</button>}
      </li>)}</ul>}
    {error && <p role="alert" className="error">{error}</p>}
  </section>;
}
