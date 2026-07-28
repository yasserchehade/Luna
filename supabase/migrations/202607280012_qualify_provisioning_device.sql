create or replace function public.authorize_managed_intelligence_device_provisioning(
  requested_device_public_key text,
  requested_challenge_id uuid,
  requested_nonce text,
  requested_authorization_signature text
)
returns table (
  household_id uuid,
  device_id uuid,
  existing_gateway_key_alias text,
  budget_scope_id uuid,
  max_budget_usd numeric
)
language plpgsql
security definer
set search_path = ''
as $$
declare
  current_household_id uuid;
  current_device_id uuid;
  device_authorization_public_key text;
  current_gateway_key_alias text;
  current_budget_scope_id uuid;
  current_max_budget_usd numeric;
begin
  if auth.jwt() ->> 'aal' <> 'aal2' then raise exception 'Authenticator verification is required'; end if;
  select membership.household_id into current_household_id
  from public.household_memberships as membership
  where membership.account_id = private.current_luna_account_id();

  select
    challenge.device_id,
    device.authorization_public_key,
    access.gateway_key_alias,
    entitlement.budget_scope_id,
    entitlement.max_budget_usd
  into
    current_device_id,
    device_authorization_public_key,
    current_gateway_key_alias,
    current_budget_scope_id,
    current_max_budget_usd
  from public.managed_intelligence_provisioning_challenges as challenge
  join public.trusted_devices as device on device.id = challenge.device_id
  join public.managed_intelligence_device_access as access on access.device_id = device.id
  join public.managed_intelligence_entitlements as entitlement on entitlement.household_id = device.household_id
  where challenge.id = requested_challenge_id
    and challenge.household_id = current_household_id
    and challenge.nonce::text = requested_nonce
    and challenge.consumed_at is null
    and challenge.expires_at > now()
    and device.public_key = requested_device_public_key
    and device.status = 'active'
    and access.status in ('pending', 'ready')
    and entitlement.status = 'active'
    and entitlement.valid_until > now()
  for update of challenge, access;
  if current_device_id is null then raise exception 'The managed-access challenge is invalid or expired'; end if;
  if not coalesce(pgsodium.crypto_sign_verify_detached(
    decode(requested_authorization_signature, 'base64'),
    convert_to(
      'luna:managed-intelligence-device:v1:'
        || private.authorization_field(current_household_id::text)
        || private.authorization_field(requested_device_public_key)
        || private.authorization_field(requested_nonce),
      'UTF8'
    ),
    decode(device_authorization_public_key, 'base64')
  ), false) then raise exception 'Trusted Device authorization is invalid'; end if;

  update public.managed_intelligence_provisioning_challenges set consumed_at = now()
  where id = requested_challenge_id;
  update public.managed_intelligence_device_access as managed_access
  set status = 'pending', updated_at = now()
  where managed_access.device_id = current_device_id;
  return query select
    current_household_id,
    current_device_id,
    current_gateway_key_alias,
    current_budget_scope_id,
    current_max_budget_usd;
end;
$$;
