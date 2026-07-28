export type HouseholdBillingContext = {
  householdId: string;
  email: string;
  externalCustomerId: string | null;
  externalSubscriptionId: string | null;
};

export type HouseholdBillingSessionDependencies = {
  authenticateOrganiser(request: Request): Promise<HouseholdBillingContext | null>;
  createCheckout(request: {
    householdId: string;
    email: string;
  }): Promise<{ transactionId: string; url: string }>;
  createCustomerPortal(request: {
    customerId: string;
    subscriptionId: string;
  }): Promise<{ url: string }>;
  recordCheckoutPending(householdId: string, transactionId: string): Promise<void>;
};

export async function handleHouseholdBillingSession(
  request: Request,
  dependencies: HouseholdBillingSessionDependencies,
): Promise<Response> {
  if (request.method !== "POST") return json({ error: "Method not allowed" }, 405);
  const context = await dependencies.authenticateOrganiser(request);
  if (!context) return json({ error: "Household Organiser authentication is required" }, 401);

  let action: unknown;
  try {
    action = (await request.json() as { action?: unknown }).action;
  } catch {
    return json({ error: "Invalid request" }, 400);
  }

  if (action === "checkout") {
    const checkout = await dependencies.createCheckout({
      householdId: context.householdId,
      email: context.email,
    });
    if (!validExternalUrl(checkout.url) || !checkout.transactionId) {
      return json({ error: "Billing session unavailable" }, 502);
    }
    await dependencies.recordCheckoutPending(context.householdId, checkout.transactionId);
    return json({ url: checkout.url }, 200);
  }

  if (action === "portal") {
    if (!context.externalCustomerId || !context.externalSubscriptionId) {
      return json({ error: "No managed subscription is available" }, 409);
    }
    const portal = await dependencies.createCustomerPortal({
      customerId: context.externalCustomerId,
      subscriptionId: context.externalSubscriptionId,
    });
    if (!validExternalUrl(portal.url)) return json({ error: "Billing session unavailable" }, 502);
    return json({ url: portal.url }, 200);
  }

  return json({ error: "Unsupported billing action" }, 400);
}

function validExternalUrl(value: string): boolean {
  try {
    return new URL(value).protocol === "https:";
  } catch {
    return false;
  }
}

function json(value: Record<string, unknown>, status: number): Response {
  return new Response(JSON.stringify(value), {
    status,
    headers: { "content-type": "application/json" },
  });
}
