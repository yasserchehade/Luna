import { useEffect, useState } from "react";
import type {
  AccountService,
  HouseholdId,
  HouseholdIntelligenceAccess,
} from "../account/accountService";
import type {
  CloudConsentScope,
  ConversationService,
  IntelligenceProviderStatus,
} from "../conversation/conversationService";
import type { TrustedDeviceService } from "../trusted-device/trustedDeviceService";

type CloudAssistanceDataService = Pick<
  ConversationService,
  "listIntelligenceProviderStatuses" | "listCloudConsentScopes"
>;

type CloudAssistanceAccountService = Pick<
  AccountService,
  | "getHouseholdIntelligenceAccess"
  | "createManagedIntelligenceCheckoutSession"
  | "createManagedIntelligenceCustomerPortalSession"
>;

export async function loadCloudAssistanceOptionsData(
  conversationService: CloudAssistanceDataService,
  accountService: CloudAssistanceAccountService,
  trustedDeviceService: Pick<TrustedDeviceService, "currentDevicePublicKey">,
  householdId: HouseholdId,
): Promise<{
  access: HouseholdIntelligenceAccess | null;
  providers: IntelligenceProviderStatus[];
  scopes: CloudConsentScope[];
  accessError: string;
  providerError: string;
  consentError: string;
}> {
  const [accessResult, providersResult, scopesResult] = await Promise.allSettled([
    trustedDeviceService.currentDevicePublicKey(householdId)
      .then((devicePublicKey) => accountService.getHouseholdIntelligenceAccess(devicePublicKey)),
    conversationService.listIntelligenceProviderStatuses(householdId),
    conversationService.listCloudConsentScopes(householdId),
  ]);
  return {
    access: accessResult.status === "fulfilled" ? accessResult.value : null,
    providers: providersResult.status === "fulfilled" ? providersResult.value : [],
    scopes: scopesResult.status === "fulfilled" ? scopesResult.value : [],
    accessError: accessResult.status === "rejected" ? String(accessResult.reason) : "",
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

export function providerSetupAvailability(
  status: Pick<IntelligenceProviderStatus, "gatewayConfigured" | "configured">,
): { enabled: boolean; statusLabel: string } {
  return {
    enabled: status.gatewayConfigured,
    statusLabel: status.configured
      ? "Connected"
      : status.gatewayConfigured
      ? "Not connected"
      : "Luna access unavailable",
  };
}

function managedAccessStatus(
  access: HouseholdIntelligenceAccess | null,
  providerConfigured: boolean,
): string {
  if (!access) return "Managed access unavailable";
  switch (access.entitlementState) {
    case "checkoutPending": return "Checkout pending";
    case "entitled": return access.deviceState === "ready" && providerConfigured
      ? "Managed access ready"
      : "Preparing this Trusted Device";
    case "paymentProblem": return "Payment needs attention";
    case "ended": return "Managed access ended";
    default: return "Managed access not included";
  }
}

export function CloudAssistanceOptions({
  accountService,
  conversationService,
  householdId,
  trustedDeviceService,
}: {
  accountService: CloudAssistanceAccountService;
  conversationService: ConversationService;
  householdId: HouseholdId;
  trustedDeviceService: Pick<TrustedDeviceService, "currentDevicePublicKey">;
}) {
  const [access, setAccess] = useState<HouseholdIntelligenceAccess | null>(null);
  const [providers, setProviders] = useState<IntelligenceProviderStatus[]>([]);
  const [scopes, setScopes] = useState<CloudConsentScope[]>([]);
  const [credentialDrafts, setCredentialDrafts] = useState<Record<string, string>>({});
  const [busyProviderId, setBusyProviderId] = useState("");
  const [providerError, setProviderError] = useState("");
  const [consentError, setConsentError] = useState("");
  const [actionError, setActionError] = useState("");
  const [accessError, setAccessError] = useState("");
  const [billingBusy, setBillingBusy] = useState(false);
  const [billingSession, setBillingSession] = useState<{ url: string; kind: "checkout" | "portal" } | null>(null);

  const refresh = async () => {
    const data = await loadCloudAssistanceOptionsData(
      conversationService,
      accountService,
      trustedDeviceService,
      householdId,
    );
    setAccess(data.access);
    setProviders(data.providers);
    setScopes(data.scopes);
    setProviderError(data.providerError);
    setConsentError(data.consentError);
    setAccessError(data.accessError);
  };

  useEffect(() => { void refresh(); }, [accountService, conversationService, householdId, trustedDeviceService]);

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

  const createBillingSession = async (kind: "checkout" | "portal") => {
    setBillingBusy(true);
    try {
      const session = kind === "checkout"
        ? await accountService.createManagedIntelligenceCheckoutSession()
        : await accountService.createManagedIntelligenceCustomerPortalSession();
      setBillingSession({ url: session.url, kind });
      setActionError("");
    } catch {
      setActionError("Luna couldn't open Paddle billing. Check the connection, then try again.");
    } finally {
      setBillingBusy(false);
    }
  };

  return <section className="cloud-assistance-options" aria-label="Cloud assistance">
    <h2>Cloud assistance</h2>
    <p className="muted">Luna stays local by default. An eligible paid or complimentary beta Household receives Luna-managed Intelligence automatically. Any Household can connect its own supported provider here, or remain local-only.</p>
    <p className="muted">For eligible Household Plans, Luna enables managed access automatically on Trusted Devices. You never need to enter a Luna access key.</p>
    {accessError && <p role="alert" className="error">Luna couldn&apos;t check this Household&apos;s managed access. Bring-your-own provider setup remains available.</p>}
    {providerError && <p role="alert" className="error">{cloudAssistanceLoadErrorMessage("providers", providerError)}</p>}
    <div className="cloud-provider-list">
      {providers.filter(({ descriptor }) => descriptor.managedByLuna).map(({ descriptor, configured }) => <article className="cloud-provider-card" key={descriptor.id}>
        <div className="cloud-provider-heading">
          <div><strong>{descriptor.name}</strong><span>{descriptor.description}</span></div>
          <small>{managedAccessStatus(access, configured)}</small>
        </div>
        {access?.entitlementSource === "complimentary" && access.entitlementState !== "ended" && <p>
          <strong>Complimentary beta</strong>
          {access.maxBudgetUsd !== null && <> · US${access.maxBudgetUsd.toFixed(2)} managed-usage cap</>}
        </p>}
        <p>Approved models: {descriptor.models.map(({ name }) => name).join(", ")}</p>
        {(access?.entitlementState !== "entitled" || access.deviceState !== "ready" || !configured) && <p className="muted">There is nothing to paste here. Luna will enable this automatically when the Household Plan and Trusted Device are eligible.</p>}
        {access?.entitlementState === "free" && <div className="default-intelligence-actions">
          <button type="button" disabled={billingBusy} onClick={() => void createBillingSession("checkout")}>Start Paddle sandbox checkout</button>
          <span className="muted">No real charge will be made in this prototype.</span>
        </div>}
        {access?.entitlementSource === "billing" && ["entitled", "paymentProblem", "ended"].includes(access.entitlementState) && <button
          type="button"
          disabled={billingBusy}
          onClick={() => void createBillingSession("portal")}
        >Manage subscription in Paddle</button>}
        {billingSession && <p>
          <a href={billingSession.url} target="_blank" rel="noreferrer">
            {billingSession.kind === "checkout" ? "Continue to Paddle sandbox" : "Continue to Paddle billing"}
          </a>
        </p>}
      </article>)}
    </div>
    <h3>Bring your own provider</h3>
    <p className="muted">The provider bills your account. Luna tests the key before saving it in this device&apos;s operating-system credential vault. Your key is used only for the provider connection you selected and never switches to Luna-funded access.</p>
    <div className="cloud-provider-list">
      {providers.filter(({ descriptor }) => !descriptor.managedByLuna).map((status) => {
        const { descriptor, gatewayConfigured, configured } = status;
        const setup = providerSetupAvailability(status);
        return <section
          className="cloud-provider-card"
          aria-label={`${descriptor.name.split(" — ")[0]} bring-your-own-key connection`}
          key={descriptor.id}
        >
          <div className="cloud-provider-heading">
            <div><strong>{descriptor.name}</strong><span>{descriptor.description}</span></div>
            <small>{setup.statusLabel}</small>
          </div>
          <p>Approved models: {descriptor.models.map(({ name }) => name).join(", ")}</p>
          {!gatewayConfigured && <p className="muted">Provider setup will be available automatically when Luna enables Cloud Assistance on this Trusted Device. There is no Luna access key to enter.</p>}
          {descriptor.authUrl && <a href={descriptor.authUrl} target="_blank" rel="noreferrer">{providerApiKeysLinkLabel(descriptor.name)}</a>}
          <label>
            {configured ? "Replacement provider API key" : "Provider API key"}
            <input
              type="password"
              autoComplete="off"
              disabled={!setup.enabled}
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
              disabled={!setup.enabled || busyProviderId === descriptor.id || !credentialDrafts[descriptor.id]?.trim()}
              onClick={() => void testAndSaveProvider(descriptor.id)}
            >{configured ? "Test and replace" : "Test and connect"}</button>
            {configured && <button
              type="button"
              disabled={busyProviderId === descriptor.id}
              onClick={() => void removeProvider(descriptor.id)}
            >Remove key</button>}
          </div>
        </section>;
      })}
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
