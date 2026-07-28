export type PaddleSubscriptionEvent = {
  eventId: string;
  eventType: "subscription.created" | "subscription.updated" | "subscription.canceled";
  occurredAt: string;
  householdId: string;
  customerId: string;
  subscriptionId: string;
  status: "trialing" | "active" | "past_due" | "paused" | "canceled";
  validUntil: string | null;
  requestLimit: number;
};

export type PaddleWebhookDependencies = {
  webhookSecret: string;
  expectedPriceId: string;
  managedRequestLimit: number;
  now(): Date;
  applySubscriptionEvent(event: PaddleSubscriptionEvent): Promise<{ applied: boolean }>;
};

const acceptedEventTypes = new Set<PaddleSubscriptionEvent["eventType"]>([
  "subscription.created",
  "subscription.updated",
  "subscription.canceled",
]);
const acceptedStatuses = new Set<PaddleSubscriptionEvent["status"]>([
  "trialing",
  "active",
  "past_due",
  "paused",
  "canceled",
]);

export async function handlePaddleWebhook(
  request: Request,
  dependencies: PaddleWebhookDependencies,
): Promise<Response> {
  if (request.method !== "POST") return json({ error: "Method not allowed" }, 405);
  const rawBody = await request.text();
  const signatureHeader = request.headers.get("paddle-signature") ?? "";
  if (!await validSignature(rawBody, signatureHeader, dependencies)) {
    return json({ error: "Invalid webhook signature" }, 401);
  }

  let event: PaddleSubscriptionEvent;
  try {
    event = parseSubscriptionEvent(
      JSON.parse(rawBody) as unknown,
      dependencies.expectedPriceId,
      dependencies.managedRequestLimit,
    );
  } catch {
    return json({ error: "Invalid webhook event" }, 400);
  }

  const result = await dependencies.applySubscriptionEvent(event);
  return json({ received: true, applied: result.applied }, 200);
}

async function validSignature(
  rawBody: string,
  signatureHeader: string,
  dependencies: Pick<PaddleWebhookDependencies, "webhookSecret" | "now">,
): Promise<boolean> {
  const fields = signatureHeader.split(";").map((part) => part.split("=", 2));
  const timestamp = fields.find(([name]) => name === "ts")?.[1];
  const signatures = fields.filter(([name]) => name === "h1").map(([, value]) => value);
  if (!timestamp || signatures.length === 0 || !/^\d+$/.test(timestamp)) return false;
  const timestampSeconds = Number(timestamp);
  const nowSeconds = Math.floor(dependencies.now().getTime() / 1_000);
  if (!Number.isSafeInteger(timestampSeconds) || Math.abs(nowSeconds - timestampSeconds) > 5) return false;

  const key = await crypto.subtle.importKey(
    "raw",
    new TextEncoder().encode(dependencies.webhookSecret),
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["sign"],
  );
  const expected = bytesToHex(new Uint8Array(await crypto.subtle.sign(
    "HMAC",
    key,
    new TextEncoder().encode(`${timestamp}:${rawBody}`),
  )));
  return signatures.some((signature) => constantTimeEqual(expected, signature));
}

function parseSubscriptionEvent(
  input: unknown,
  expectedPriceId: string,
  requestLimit: number,
): PaddleSubscriptionEvent {
  if (!input || typeof input !== "object") throw new Error("event");
  const event = input as Record<string, unknown>;
  if (
    typeof event.event_id !== "string"
    || typeof event.event_type !== "string"
    || !acceptedEventTypes.has(event.event_type as PaddleSubscriptionEvent["eventType"])
    || typeof event.occurred_at !== "string"
    || Number.isNaN(Date.parse(event.occurred_at))
    || !event.data
    || typeof event.data !== "object"
  ) throw new Error("event");

  const data = event.data as Record<string, unknown>;
  const customData = data.custom_data as Record<string, unknown> | null;
  const billingPeriod = data.current_billing_period as Record<string, unknown> | null;
  const items = data.items;
  if (
    typeof data.id !== "string"
    || typeof data.customer_id !== "string"
    || typeof data.status !== "string"
    || !acceptedStatuses.has(data.status as PaddleSubscriptionEvent["status"])
    || !customData
    || typeof customData.household_id !== "string"
    || !/^[0-9a-f-]{36}$/i.test(customData.household_id)
    || !Array.isArray(items)
    || !items.some((item) => {
      if (!item || typeof item !== "object") return false;
      const price = (item as Record<string, unknown>).price;
      return price && typeof price === "object"
        && (price as Record<string, unknown>).id === expectedPriceId;
    })
    || !Number.isSafeInteger(requestLimit)
    || requestLimit <= 0
  ) throw new Error("subscription");

  const validUntil = billingPeriod?.ends_at;
  if (validUntil !== undefined && validUntil !== null && (
    typeof validUntil !== "string" || Number.isNaN(Date.parse(validUntil))
  )) throw new Error("billing period");

  return {
    eventId: event.event_id,
    eventType: event.event_type as PaddleSubscriptionEvent["eventType"],
    occurredAt: event.occurred_at,
    householdId: customData.household_id,
    customerId: data.customer_id,
    subscriptionId: data.id,
    status: data.status as PaddleSubscriptionEvent["status"],
    validUntil: validUntil as string | null | undefined ?? null,
    requestLimit,
  };
}

function bytesToHex(bytes: Uint8Array): string {
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
}

function constantTimeEqual(left: string, right: string): boolean {
  if (left.length !== right.length) return false;
  let difference = 0;
  for (let index = 0; index < left.length; index += 1) {
    difference |= left.charCodeAt(index) ^ right.charCodeAt(index);
  }
  return difference === 0;
}

function json(value: Record<string, unknown>, status: number): Response {
  return new Response(JSON.stringify(value), {
    status,
    headers: { "content-type": "application/json" },
  });
}
