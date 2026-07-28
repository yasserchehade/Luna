create table public.managed_intelligence_device_access (
  device_id uuid primary key references public.trusted_devices(id) on delete cascade,
  household_id uuid not null references public.households(id) on delete cascade,
  status text not null check (status in ('pending', 'ready', 'revoked')),
  gateway_key_alias text check (gateway_key_alias is null or char_length(gateway_key_alias) > 0),
  updated_at timestamptz not null default now(),
  unique (household_id, device_id)
);

create table public.managed_intelligence_provisioning_challenges (
  id uuid primary key default gen_random_uuid(),
  household_id uuid not null references public.households(id) on delete cascade,
  device_id uuid not null references public.trusted_devices(id) on delete cascade,
  nonce uuid not null default gen_random_uuid(),
  expires_at timestamptz not null default (now() + interval '5 minutes'),
  consumed_at timestamptz,
  created_at timestamptz not null default now()
);

alter table public.managed_intelligence_device_access enable row level security;
alter table public.managed_intelligence_provisioning_challenges enable row level security;
revoke all on public.managed_intelligence_device_access from anon, authenticated;
revoke all on public.managed_intelligence_provisioning_challenges from anon, authenticated;

create or replace function private.synchronize_managed_intelligence_device_access()
returns trigger
language plpgsql
security definer
set search_path = ''
as $$
begin
  if new.status = 'active' and new.valid_until > now() and new.requests_used < new.request_limit then
    insert into public.managed_intelligence_device_access (device_id, household_id, status)
    select device.id, device.household_id, 'pending'
    from public.trusted_devices as device
    where device.household_id = new.household_id and device.status = 'active'
    on conflict (device_id) do update set
      status = 'pending', gateway_key_alias = null, updated_at = now();
  else
    update public.managed_intelligence_device_access
    set status = 'revoked', gateway_key_alias = null, updated_at = now()
    where household_id = new.household_id;
  end if;
  return new;
end;
$$;

create trigger synchronize_managed_intelligence_device_access_after_entitlement
after insert or update of status, valid_until, request_limit, requests_used
on public.managed_intelligence_entitlements
for each row execute function private.synchronize_managed_intelligence_device_access();

create or replace function private.initialize_managed_intelligence_device_access()
returns trigger
language plpgsql
security definer
set search_path = ''
as $$
begin
  if new.status = 'active' and exists (
    select 1 from public.managed_intelligence_entitlements as entitlement
    where entitlement.household_id = new.household_id
      and entitlement.status = 'active'
      and entitlement.valid_until > now()
      and entitlement.requests_used < entitlement.request_limit
  ) then
    insert into public.managed_intelligence_device_access (device_id, household_id, status)
    values (new.id, new.household_id, 'pending')
    on conflict (device_id) do update set
      status = 'pending', gateway_key_alias = null, updated_at = now();
  elsif new.status = 'revoked' then
    update public.managed_intelligence_device_access
    set status = 'revoked', gateway_key_alias = null, updated_at = now()
    where device_id = new.id;
  end if;
  return new;
end;
$$;

create trigger initialize_managed_intelligence_device_access_after_device
after insert or update of status on public.trusted_devices
for each row execute function private.initialize_managed_intelligence_device_access();

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
  if auth.jwt() ->> 'aal' <> 'aal2' then
    raise exception 'Authenticator verification is required';
  end if;
  select membership.household_id into current_household_id
  from public.household_memberships as membership
  where membership.account_id = private.current_luna_account_id();
  select device.id into current_device_id
  from public.trusted_devices as device
  join public.managed_intelligence_device_access as access on access.device_id = device.id
  join public.managed_intelligence_entitlements as entitlement
    on entitlement.household_id = device.household_id
  where device.household_id = current_household_id
    and device.public_key = requested_device_public_key
    and device.status = 'active'
    and access.status in ('pending', 'ready')
    and entitlement.status = 'active'
    and entitlement.valid_until > now()
    and entitlement.requests_used < entitlement.request_limit;
  if current_device_id is null then
    raise exception 'This Trusted Device is not eligible for managed access';
  end if;

  return query
  insert into public.managed_intelligence_provisioning_challenges (household_id, device_id)
  values (current_household_id, current_device_id)
  returning id, nonce::text, managed_intelligence_provisioning_challenges.expires_at;
end;
$$;

create or replace function public.authorize_managed_intelligence_device_provisioning(
  requested_device_public_key text,
  requested_challenge_id uuid,
  requested_nonce text,
  requested_authorization_signature text
)
returns table (household_id uuid, device_id uuid)
language plpgsql
security definer
set search_path = ''
as $$
declare
  current_household_id uuid;
  current_device_id uuid;
  device_authorization_public_key text;
begin
  if auth.jwt() ->> 'aal' <> 'aal2' then
    raise exception 'Authenticator verification is required';
  end if;
  select membership.household_id into current_household_id
  from public.household_memberships as membership
  where membership.account_id = private.current_luna_account_id();

  select challenge.device_id, device.authorization_public_key
    into current_device_id, device_authorization_public_key
  from public.managed_intelligence_provisioning_challenges as challenge
  join public.trusted_devices as device on device.id = challenge.device_id
  join public.managed_intelligence_device_access as access on access.device_id = device.id
  join public.managed_intelligence_entitlements as entitlement
    on entitlement.household_id = device.household_id
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
    and entitlement.requests_used < entitlement.request_limit
  for update of challenge;
  if current_device_id is null then
    raise exception 'The managed-access challenge is invalid or expired';
  end if;
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
  ), false) then
    raise exception 'Trusted Device authorization is invalid';
  end if;

  update public.managed_intelligence_provisioning_challenges
  set consumed_at = now()
  where id = requested_challenge_id;
  return query select current_household_id, current_device_id;
end;
$$;

create or replace function public.record_managed_intelligence_device_access(
  requested_household_id uuid,
  requested_device_id uuid,
  requested_status text,
  requested_gateway_key_alias text
)
returns void
language plpgsql
security definer
set search_path = ''
as $$
begin
  if auth.role() <> 'service_role' then
    raise exception 'Operator authority is required';
  end if;
  if requested_status not in ('ready', 'revoked') then
    raise exception 'Unsupported managed device access status';
  end if;
  if requested_status = 'ready' and trim(coalesce(requested_gateway_key_alias, '')) = '' then
    raise exception 'A gateway key alias is required';
  end if;
  update public.managed_intelligence_device_access as access
  set status = requested_status,
      gateway_key_alias = case when requested_status = 'ready' then requested_gateway_key_alias else null end,
      updated_at = now()
  from public.trusted_devices as device
  where access.device_id = requested_device_id
    and access.household_id = requested_household_id
    and device.id = access.device_id
    and (
      requested_status = 'revoked'
      or (
        device.status = 'active'
        and exists (
          select 1 from public.managed_intelligence_entitlements as entitlement
          where entitlement.household_id = requested_household_id
            and entitlement.status = 'active'
            and entitlement.valid_until > now()
            and entitlement.requests_used < entitlement.request_limit
        )
      )
    );
  if not found then raise exception 'Managed Trusted Device access is not eligible'; end if;
end;
$$;

create or replace function public.current_household_intelligence_access()
returns table (
  household_id uuid,
  plan_code text,
  access_state text,
  entitlement_source text,
  request_limit integer,
  requests_used integer,
  valid_until timestamptz
)
language sql
stable
security definer
set search_path = ''
as $$
  select
    plan.household_id,
    plan.plan_code,
    case
      when subscription.status = 'checkout_pending' then 'checkout_pending'
      when subscription.status = 'past_due' then 'payment_problem'
      when subscription.status in ('paused', 'canceled') then 'ended'
      when entitlement.status = 'active'
        and entitlement.valid_until > now()
        and entitlement.requests_used < entitlement.request_limit
        and exists (
          select 1 from public.managed_intelligence_device_access as access
          where access.household_id = plan.household_id and access.status = 'ready'
        ) then 'ready'
      when entitlement.status = 'active'
        and entitlement.valid_until > now()
        and entitlement.requests_used < entitlement.request_limit then 'provisioning'
      when entitlement.household_id is not null then 'ended'
      else 'free'
    end,
    entitlement.source,
    entitlement.request_limit,
    coalesce(entitlement.requests_used, 0),
    entitlement.valid_until
  from public.household_plans as plan
  join public.household_memberships as membership
    on membership.household_id = plan.household_id
  left join public.managed_intelligence_entitlements as entitlement
    on entitlement.household_id = plan.household_id
  left join public.billing_subscriptions as subscription
    on subscription.household_id = plan.household_id
  where membership.account_id = private.current_luna_account_id()
$$;

revoke all on function public.begin_managed_intelligence_device_provisioning(text) from public;
revoke all on function public.authorize_managed_intelligence_device_provisioning(text, uuid, text, text) from public;
revoke all on function public.record_managed_intelligence_device_access(uuid, uuid, text, text) from public;
grant execute on function public.begin_managed_intelligence_device_provisioning(text) to authenticated;
grant execute on function public.authorize_managed_intelligence_device_provisioning(text, uuid, text, text) to authenticated;
grant execute on function public.record_managed_intelligence_device_access(uuid, uuid, text, text) to service_role;
