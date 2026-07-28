import { useEffect, useState } from "react";
import type {
  CloudConsentScope,
  ConversationService,
  IntelligenceProviderStatus,
} from "../conversation/conversationService";

type CloudAssistanceDataService = Pick<
  ConversationService,
  "listIntelligenceProviderStatuses" | "listCloudConsentScopes"
>;

export async function loadCloudAssistanceOptionsData(
  conversationService: CloudAssistanceDataService,
  householdId: string,
): Promise<{
  providers: IntelligenceProviderStatus[];
  scopes: CloudConsentScope[];
  providerError: string;
  consentError: string;
}> {
  const [providersResult, scopesResult] = await Promise.allSettled([
    conversationService.listIntelligenceProviderStatuses(householdId),
    conversationService.listCloudConsentScopes(householdId),
  ]);
  return {
    providers: providersResult.status === "fulfilled" ? providersResult.value : [],
    scopes: scopesResult.status === "fulfilled" ? scopesResult.value : [],
    providerError: providersResult.status === "rejected" ? String(providersResult.reason) : "",
    consentError: scopesResult.status === "rejected" ? String(scopesResult.reason) : "",
  };
}

export function cloudAssistanceLoadErrorMessage(
  area: "providers" | "consents",
  _technicalReason: string,
): string {
  if (area === "providers") {
    return "Luna couldn't check provider connections on this device. Lock and unlock Luna, then try again.";
  }
  return "Luna couldn't open older Consent Grants on this device. They will not be reused unless Luna can verify them. Provider setup remains available.";
}

export function providerApiKeysLinkLabel(providerName: string): string {
  return `${providerName.split(" — ")[0]} API keys`;
}

export function CloudAssistanceOptions({ conversationService, householdId }: { conversationService: ConversationService; householdId: string }) {
  const [providers, setProviders] = useState<IntelligenceProviderStatus[]>([]);
  const [scopes, setScopes] = useState<CloudConsentScope[]>([]);
  const [credentialDrafts, setCredentialDrafts] = useState<Record<string, string>>({});
  const [busyProviderId, setBusyProviderId] = useState("");
  const [providerError, setProviderError] = useState("");
  const [consentError, setConsentError] = useState("");
  const [actionError, setActionError] = useState("");

  const refresh = async () => {
    const data = await loadCloudAssistanceOptionsData(conversationService, householdId);
    setProviders(data.providers);
    setScopes(data.scopes);
    setProviderError(data.providerError);
    setConsentError(data.consentError);
  };

  useEffect(() => { void refresh(); }, [conversationService, householdId]);

  const revoke = async (scopeId: number) => {
    try {
      await conversationService.revokeCloudConsentScope(householdId, scopeId);
      setActionError("");
      await refresh();
    } catch {
      setActionError("Luna couldn't revoke this Consent Grant. Lock and unlock Luna, then try again.");
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
      setActionError("");
      await refresh();
    } catch {
      setActionError("Luna couldn't verify this provider key. Check the key and connection, then try again.");
    } finally {
      setBusyProviderId("");
    }
  };

  const removeProvider = async (providerId: string) => {
    setBusyProviderId(providerId);
    try {
      await conversationService.clearIntelligenceProviderCredential(householdId, providerId);
      setCredentialDrafts((current) => ({ ...current, [providerId]: "" }));
      setActionError("");
      await refresh();
    } catch {
      setActionError("Luna couldn't remove this provider key. Lock and unlock Luna, then try again.");
    } finally {
      setBusyProviderId("");
    }
  };

  return <section className="cloud-assistance-options" aria-label="Cloud assistance">
    <h2>Cloud assistance</h2>
    <p className="muted">Luna stays local by default. An eligible paid Household receives Luna-managed Intelligence automatically. A free or paid Household can connect its own supported provider here, or remain local-only.</p>
    <p className="muted">For eligible Household Plans, Luna enables managed access automatically on Trusted Devices. You never need to enter a Luna access key.</p>
    {providerError && <p role="alert" className="error">{cloudAssistanceLoadErrorMessage("providers", providerError)}</p>}
    <div className="cloud-provider-list">
      {providers.filter(({ descriptor }) => descriptor.managedByLuna).map(({ descriptor, configured }) => <article className="cloud-provider-card" key={descriptor.id}>
        <div className="cloud-provider-heading">
          <div><strong>{descriptor.name}</strong><span>{descriptor.description}</span></div>
          <small>{configured ? "Managed access ready" : "Managed access unavailable"}</small>
        </div>
        <p>Approved models: {descriptor.models.map(({ name }) => name).join(", ")}</p>
        {!configured && <p className="muted">There is nothing to paste here. Luna will enable this automatically when the Household Plan and Trusted Device are eligible.</p>}
      </article>)}
    </div>
    <h3>Bring your own provider</h3>
    <p className="muted">The provider bills your account. Luna tests the key before saving it in this device&apos;s operating-system credential vault. Your key is used only for the provider connection you selected and never switches to Luna-funded access.</p>
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
        {descriptor.authUrl && <a href={descriptor.authUrl} target="_blank" rel="noreferrer">{providerApiKeysLinkLabel(descriptor.name)}</a>}
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
    {consentError
      ? <p role="alert" className="error">{cloudAssistanceLoadErrorMessage("consents", consentError)}</p>
      : scopes.length === 0
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
    {actionError && <p role="alert" className="error">{actionError}</p>}
  </section>;
}
