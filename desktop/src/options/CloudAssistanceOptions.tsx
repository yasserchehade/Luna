import { useEffect, useState } from "react";
import type {
  CloudConsentScope,
  ConversationService,
  IntelligenceProviderStatus,
} from "../conversation/conversationService";

export function CloudAssistanceOptions({ conversationService, householdId }: { conversationService: ConversationService; householdId: string }) {
  const [providers, setProviders] = useState<IntelligenceProviderStatus[]>([]);
  const [scopes, setScopes] = useState<CloudConsentScope[]>([]);
  const [gatewayCredential, setGatewayCredential] = useState("");
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

  const saveGatewayCredential = async () => {
    const credential = gatewayCredential.trim();
    if (!credential) return;
    try {
      await conversationService.setLunaGatewayCredential(householdId, credential);
      setGatewayCredential("");
      await refresh();
    } catch (reason) {
      setError(String(reason));
    }
  };

  const clearGatewayCredential = async () => {
    try {
      await conversationService.clearLunaGatewayCredential(householdId);
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

  const gatewayConfigured = providers.some(({ configured }) => configured);

  return <section className="cloud-assistance-options" aria-label="Cloud assistance">
    <h2>Cloud assistance</h2>
    <p className="muted">Luna stays local by default. Luna selects an exact Intelligence Provider and model before requesting a provider-specific Consent Grant. Upstream provider credentials remain on Luna&apos;s managed gateway and never reach this desktop.</p>
    <div className="cloud-credential-entry">
      <label>Luna gateway access credential<input type="password" value={gatewayCredential} onChange={(event) => setGatewayCredential(event.target.value)} autoComplete="off" /></label>
      <button type="button" disabled={!gatewayCredential.trim()} onClick={() => void saveGatewayCredential()}>Save in operating-system vault</button>
      {gatewayConfigured && <button type="button" onClick={() => void clearGatewayCredential()}>Remove gateway access</button>}
    </div>
    <p className="muted">This narrow, revocable credential authenticates the Trusted Device to Luna&apos;s gateway. It is not an OpenAI API key and never enters ordinary configuration or History.</p>
    <div className="cloud-provider-list">
      {providers.map(({ descriptor, configured }) => <article className="cloud-provider-card" key={descriptor.id}>
        <div className="cloud-provider-heading">
          <div><strong>{descriptor.name}</strong><span>{descriptor.description}</span></div>
          <small>{configured ? "Gateway ready" : "Gateway access unavailable"}</small>
        </div>
        <p>Approved models: {descriptor.models.map(({ name }) => name).join(", ")}</p>
      </article>)}
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
