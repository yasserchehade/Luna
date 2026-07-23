import { useEffect, useState } from "react";
import type { CloudConsentScope, ConversationService } from "../conversation/conversationService";

export function CloudAssistanceOptions({ conversationService, householdId }: { conversationService: ConversationService; householdId: string }) {
  const [scopes, setScopes] = useState<CloudConsentScope[]>([]);
  const [provider, setProvider] = useState<{ id: string; name: string } | null>(null);
  const [error, setError] = useState("");

  const refresh = () => conversationService.listCloudConsentScopes(householdId).then(setScopes).catch((reason) => setError(String(reason)));
  useEffect(() => {
    void refresh();
    void conversationService.listIntelligenceProviders().then((providers) => {
      if (providers[0]) setProvider({ id: providers[0].id, name: providers[0].name });
    }).catch(() => undefined);
  }, [conversationService, householdId]);

  const grant = async () => {
    setError("");
    try {
      if (!provider) return;
      await conversationService.grantCloudConsentScope(householdId, provider.id, "document-evaluation", ["documentType", "serviceProvider", "addressee", "property", "account", "amount", "relevantDates"]);
      await refresh();
    } catch (reason) {
      setError(String(reason));
    }
  };

  const revoke = async (scopeId: number) => {
    setError("");
    try {
      await conversationService.revokeCloudConsentScope(householdId, scopeId);
      await refresh();
    } catch (reason) {
      setError(String(reason));
    }
  };

  return <section className="cloud-assistance-options" aria-label="Cloud assistance">
    <h2>Cloud assistance</h2>
    <p className="muted">Luna stays local by default. A scoped grant is limited to one named provider and purpose, and can be revoked here.</p>
    <div className="cloud-provider-card">
      <strong>{provider?.name ?? "No provider available"}</strong>
      <span>Document evaluation</span>
      <button type="button" disabled={!provider} onClick={() => void grant}>Allow future document evaluations</button>
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
