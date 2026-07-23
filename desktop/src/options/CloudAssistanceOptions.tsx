import { useEffect, useState } from "react";
import type {
  CloudConsentScope,
  ConversationService,
  IntelligenceProviderStatus,
} from "../conversation/conversationService";

const evaluationFields = [
  "documentType",
  "serviceProvider",
  "addressee",
  "property",
  "account",
  "amount",
  "relevantDates",
];

export function CloudAssistanceOptions({ conversationService, householdId }: { conversationService: ConversationService; householdId: string }) {
  const [providers, setProviders] = useState<IntelligenceProviderStatus[]>([]);
  const [scopes, setScopes] = useState<CloudConsentScope[]>([]);
  const [credentialDrafts, setCredentialDrafts] = useState<Record<string, string>>({});
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

  const saveCredential = async (providerId: string) => {
    const credential = credentialDrafts[providerId]?.trim();
    if (!credential) return;
    try {
      await conversationService.setCloudProviderCredential(householdId, providerId, credential);
      setCredentialDrafts((current) => ({ ...current, [providerId]: "" }));
      await refresh();
    } catch (reason) {
      setError(String(reason));
    }
  };

  const clearCredential = async (providerId: string) => {
    try {
      await conversationService.clearCloudProviderCredential(householdId, providerId);
      await refresh();
    } catch (reason) {
      setError(String(reason));
    }
  };

  const grant = async (providerId: string) => {
    try {
      await conversationService.grantCloudConsentScope(householdId, providerId, "document-evaluation", evaluationFields);
      await refresh();
    } catch (reason) {
      setError(String(reason));
    }
  };

  const revoke = async (scopeId: number) => {
    try {
      await conversationService.revokeCloudConsentScope(householdId, scopeId);
      await refresh();
    } catch (reason) {
      setError(String(reason));
    }
  };

  return <section className="cloud-assistance-options" aria-label="Cloud assistance">
    <h2>Cloud assistance</h2>
    <p className="muted">Luna stays local by default. Bring your own provider key; it is kept in this device&apos;s credential vault and never written to Household history.</p>
    <div className="cloud-provider-list">
      {providers.map(({ descriptor, configured }) => <article className="cloud-provider-card" key={descriptor.id}>
        <div className="cloud-provider-heading">
          <div><strong>{descriptor.name}</strong><span>{descriptor.description}</span></div>
          <small>{configured ? "Connected" : "Not connected"}</small>
        </div>
        {descriptor.authUrl && <a href={descriptor.authUrl} target="_blank" rel="noreferrer">Open {descriptor.name} API keys</a>}
        {descriptor.authUrl && !configured && <div className="cloud-credential-entry">
          <label>API key<input type="password" value={credentialDrafts[descriptor.id] ?? ""} onChange={(event) => setCredentialDrafts((current) => ({ ...current, [descriptor.id]: event.target.value }))} autoComplete="off" /></label>
          <button type="button" disabled={!credentialDrafts[descriptor.id]?.trim()} onClick={() => void saveCredential(descriptor.id)}>Save in vault</button>
        </div>}
        {descriptor.authUrl && configured && <button type="button" onClick={() => void clearCredential(descriptor.id)}>Disconnect and remove key</button>}
        <button type="button" onClick={() => void grant(descriptor.id)}>Allow future document evaluations</button>
      </article>)}
    </div>
    <h3>Consent scopes</h3>
    {scopes.length === 0
      ? <p className="muted">No future cloud consent is active.</p>
      : <ul className="consent-scope-list">{scopes.map((scope) => <li key={scope.id} className={scope.revoked ? "revoked" : undefined}>
        <div><strong>{scope.providerId}</strong><span>{scope.purpose} · {scope.fields.join(", ")}</span><small>{scope.revoked ? "Revoked" : "Active"} · {scope.createdAt}</small></div>
        {!scope.revoked && <button type="button" onClick={() => void revoke(scope.id)}>Revoke</button>}
      </li>)}</ul>}
    {error && <p role="alert" className="error">{error}</p>}
  </section>;
}
