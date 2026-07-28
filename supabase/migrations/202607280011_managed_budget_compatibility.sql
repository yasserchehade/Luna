create or replace function public.begin_managed_intelligence_device_provisioning(
  requested_device_public_key text
)
returns table (challenge_id uuid, challenge_nonce text, expires_at timestamptz)
language plpgsql
security definer
set search_path = ''
as $$
declare
  current_household_id uuid;
  current_device_id uuid;
begin
  if auth.jwt() ->> 'aal' <> 'aal2' then raise exception 'Authenticator verification is required'; end if;
  select membership.household_id into current_household_id
  from public.household_memberships as membership
  where membership.account_id = private.current_luna_account_id();
  select device.id into current_device_id
  from public.trusted_devices as device
  join public.managed_intelligence_device_access as access on access.device_id = device.id
  join public.managed_intelligence_entitlements as entitlement on entitlement.household_id = device.household_id
  where device.household_id = current_household_id
    and device.public_key = requested_device_public_key
    and device.status = 'active'
    and access.status in ('pending', 'ready')
    and entitlement.status = 'active'
    and entitlement.valid_until > now();
  if current_device_id is null then raise exception 'This Trusted Device is not eligible for managed access'; end if;

  return query
  insert into public.managed_intelligence_provisioning_challenges (household_id, device_id)
  values (current_household_id, current_device_id)
  returning id, nonce::text, managed_intelligence_provisioning_challenges.expires_at;
end;
$$;
