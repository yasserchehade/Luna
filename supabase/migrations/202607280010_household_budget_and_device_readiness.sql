drop function if exists public.current_household_intelligence_access();
drop trigger if exists synchronize_managed_intelligence_device_access_after_entitlement
  on public.managed_intelligence_entitlements;

alter table public.managed_intelligence_entitlements
  rename column request_limit to max_budget_usd;

alter table public.managed_intelligence_entitlements
  alter column max_budget_usd type numeric(8, 2) using max_budget_usd::numeric(8, 2),
  drop column requests_used,
  add column budget_scope_id uuid not null default gen_random_uuid();

update public.managed_intelligence_entitlements
set max_budget_usd = 1.00
where max_budget_usd > 100;

alter table public.managed_intelligence_entitlements
  add constraint managed_intelligence_entitlements_budget_bounded
  check (max_budget_usd > 0 and max_budget_usd <= 100);

alter table public.managed_intelligence_device_access
  add column credential_expires_at timestamptz;

drop function if exists public.grant_complimentary_managed_intelligence(uuid, integer, timestamptz);

create function public.grant_complimentary_managed_intelligence(
  requested_household_id uuid,
  requested_max_budget_usd numeric,
  requested_valid_until timestamptz
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
  if requested_max_budget_usd is null
    or requested_max_budget_usd <= 0
    or requested_max_budget_usd > 100 then
    raise exception 'A bounded managed-intelligence budget is required';
  end if;
  if requested_valid_until is null or requested_valid_until <= now() then
    raise exception 'A future entitlement expiry is required';
  end if;
  if not exists (
    select 1 from public.households as household
    where household.id = requested_household_id
  ) then
    raise exception 'Household not found';
  end if;

  insert into public.managed_intelligence_entitlements (
    household_id,
    source,
    status,
    max_budget_usd,
    valid_from,
    valid_until,
    updated_at,
    budget_scope_id
  ) values (
    requested_household_id,
    'complimentary',
    'active',
    requested_max_budget_usd,
    now(),
    requested_valid_until,
    now(),
    gen_random_uuid()
  )
  on conflict (household_id) do update set
    source = excluded.source,
    status = excluded.status,
    max_budget_usd = excluded.max_budget_usd,
    valid_from = excluded.valid_from,
    valid_until = excluded.valid_until,
    updated_at = excluded.updated_at,
    budget_scope_id = excluded.budget_scope_id;

  update public.household_plans
  set plan_code = 'managed', updated_at = now()
  where household_id = requested_household_id;
end;
$$;

drop function if exists public.apply_paddle_subscription_event(
  text, text, timestamptz, uuid, text, text, text, timestamptz, integer
);

create function public.apply_paddle_subscription_event(
  requested_event_id text,
  requested_event_type text,
  requested_occurred_at timestamptz,
  requested_household_id uuid,
  requested_customer_id text,
  requested_subscription_id text,
  requested_status text,
  requested_valid_until timestamptz,
  requested_max_budget_usd numeric
)
returns boolean
language plpgsql
security definer
set search_path = ''
as $$
declare
  event_inserted_count integer;
  subscription_applied uuid;
begin
  if auth.role() <> 'service_role' then raise exception 'Operator authority is required'; end if;
  if requested_event_type not in ('subscription.created', 'subscription.updated', 'subscription.canceled') then
    raise exception 'Unsupported Paddle event type';
  end if;
  if requested_status not in ('trialing', 'active', 'past_due', 'paused', 'canceled') then
    raise exception 'Unsupported Paddle subscription status';
  end if;
  if trim(coalesce(requested_event_id, '')) = ''
    or trim(coalesce(requested_customer_id, '')) = ''
    or trim(coalesce(requested_subscription_id, '')) = '' then
    raise exception 'Paddle billing identifiers are required';
  end if;
  if requested_max_budget_usd is null
    or requested_max_budget_usd <= 0
    or requested_max_budget_usd > 100 then
    raise exception 'A bounded managed-intelligence budget is required';
  end if;
  if requested_status in ('trialing', 'active')
    and (requested_valid_until is null or requested_valid_until <= requested_occurred_at) then
    raise exception 'An active subscription requires a future billing-period end';
  end if;

  perform pg_catalog.pg_advisory_xact_lock(
    pg_catalog.hashtextextended(requested_household_id::text, 0)
  );
  insert into public.billing_events (
    billing_provider, external_event_id, household_id, external_subscription_id, event_type, occurred_at
  ) values (
    'paddle', requested_event_id, requested_household_id, requested_subscription_id,
    requested_event_type, requested_occurred_at
  ) on conflict (billing_provider, external_event_id) do nothing;
  get diagnostics event_inserted_count = row_count;
  if event_inserted_count = 0 then return false; end if;

  insert into public.billing_subscriptions (
    household_id, billing_provider, external_customer_id, external_subscription_id,
    status, last_event_at, updated_at
  ) values (
    requested_household_id, 'paddle', requested_customer_id, requested_subscription_id,
    requested_status, requested_occurred_at, now()
  )
  on conflict (household_id) do update set
    external_customer_id = excluded.external_customer_id,
    external_subscription_id = excluded.external_subscription_id,
    status = excluded.status,
    last_event_at = excluded.last_event_at,
    updated_at = excluded.updated_at
  where public.billing_subscriptions.last_event_at < excluded.last_event_at
  returning household_id into subscription_applied;
  if subscription_applied is null then return false; end if;

  if requested_status in ('trialing', 'active') then
    insert into public.managed_intelligence_entitlements (
      household_id, source, status, max_budget_usd, valid_from, valid_until,
      updated_at, budget_scope_id
    ) values (
      requested_household_id, 'billing', 'active', requested_max_budget_usd,
      requested_occurred_at, requested_valid_until, now(), gen_random_uuid()
    )
    on conflict (household_id) do update set
      source = excluded.source,
      status = excluded.status,
      max_budget_usd = excluded.max_budget_usd,
      valid_from = excluded.valid_from,
      valid_until = excluded.valid_until,
      updated_at = excluded.updated_at,
      budget_scope_id = case
        when public.managed_intelligence_entitlements.source = 'billing'
          and public.managed_intelligence_entitlements.status = 'active'
          and public.managed_intelligence_entitlements.valid_until = excluded.valid_until
          then public.managed_intelligence_entitlements.budget_scope_id
        else excluded.budget_scope_id
      end;
    update public.household_plans set plan_code = 'managed', updated_at = now()
    where household_id = requested_household_id;
  else
    update public.managed_intelligence_entitlements
    set status = 'revoked', updated_at = now()
    where household_id = requested_household_id and source = 'billing';
    if requested_status in ('paused', 'canceled') then
      update public.household_plans set plan_code = 'free', updated_at = now()
      where household_id = requested_household_id;
    end if;
  end if;

  update public.billing_events set applied = true
  where billing_provider = 'paddle' and external_event_id = requested_event_id;
  return true;
end;
$$;

create or replace function private.synchronize_managed_intelligence_device_access()
returns trigger
language plpgsql
security definer
set search_path = ''
as $$
begin
  if new.status = 'active' and new.valid_until > now() then
    insert into public.managed_intelligence_device_access (device_id, household_id, status)
    select device.id, device.household_id, 'pending'
    from public.trusted_devices as device
    where device.household_id = new.household_id and device.status = 'active'
    on conflict (device_id) do update set
      status = case
        when public.managed_intelligence_device_access.status = 'revoked'
          and public.managed_intelligence_device_access.gateway_key_alias is null then 'pending'
        when tg_op = 'UPDATE' and old.budget_scope_id is distinct from new.budget_scope_id
          then 'revoked'
        else public.managed_intelligence_device_access.status
      end,
      updated_at = now();
  else
    update public.managed_intelligence_device_access
    set status = 'revoked', updated_at = now()
    where household_id = new.household_id and status <> 'revoked';
  end if;
  return new;
end;
$$;

create trigger synchronize_managed_intelligence_device_access_after_entitlement
after insert or update of status, valid_until, max_budget_usd, budget_scope_id
on public.managed_intelligence_entitlements
for each row execute function private.synchronize_managed_intelligence_device_access();

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
      and entitlement.status = 'active' and entitlement.valid_until > now()
  ) then
    insert into public.managed_intelligence_device_access (device_id, household_id, status)
    values (new.id, new.household_id, 'pending')
    on conflict (device_id) do update set
      status = case
        when public.managed_intelligence_device_access.status = 'revoked'
          and public.managed_intelligence_device_access.gateway_key_alias is null then 'pending'
        else public.managed_intelligence_device_access.status
      end,
      updated_at = now();
  elsif new.status = 'revoked' then
    update public.managed_intelligence_device_access
    set status = 'revoked', updated_at = now()
    where device_id = new.id and status <> 'revoked';
  end if;
  return new;
end;
$$;

drop function if exists public.authorize_managed_intelligence_device_provisioning(text, uuid, text, text);

create function public.authorize_managed_intelligence_device_provisioning(
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

drop function if exists public.record_managed_intelligence_device_access(uuid, uuid, text, text);

create function public.record_managed_intelligence_device_access(
  requested_household_id uuid,
  requested_device_id uuid,
  requested_status text,
  requested_gateway_key_alias text,
  requested_credential_expires_at timestamptz
)
returns void
language plpgsql
security definer
set search_path = ''
as $$
begin
  if auth.role() <> 'service_role' then raise exception 'Operator authority is required'; end if;
  if requested_status not in ('ready', 'revoked') then raise exception 'Unsupported managed device access status'; end if;
  if requested_status = 'ready' and (
    trim(coalesce(requested_gateway_key_alias, '')) = ''
    or requested_credential_expires_at is null
    or requested_credential_expires_at <= now()
  ) then raise exception 'A current gateway credential is required'; end if;

  update public.managed_intelligence_device_access as access
  set status = requested_status,
      gateway_key_alias = case when requested_status = 'ready' then requested_gateway_key_alias else access.gateway_key_alias end,
      credential_expires_at = case when requested_status = 'ready' then requested_credential_expires_at else access.credential_expires_at end,
      gateway_revoked_at = case when requested_status = 'ready' then null else access.gateway_revoked_at end,
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
            and entitlement.status = 'active' and entitlement.valid_until > now()
        )
      )
    );
  if not found then raise exception 'Managed Trusted Device access is not eligible'; end if;
end;
$$;

create or replace function public.record_managed_intelligence_gateway_revoked(
  requested_household_id uuid,
  requested_device_id uuid
)
returns void
language plpgsql
security definer
set search_path = ''
as $$
begin
  if auth.role() <> 'service_role' then raise exception 'Operator authority is required'; end if;
  update public.managed_intelligence_device_access as access
  set gateway_key_alias = null,
      credential_expires_at = null,
      gateway_revoked_at = now(),
      status = case
        when device.status = 'active' and exists (
          select 1 from public.managed_intelligence_entitlements as entitlement
          where entitlement.household_id = requested_household_id
            and entitlement.status = 'active' and entitlement.valid_until > now()
        ) then 'pending'
        else 'revoked'
      end,
      updated_at = now()
  from public.trusted_devices as device
  where access.household_id = requested_household_id
    and access.device_id = requested_device_id
    and device.id = access.device_id
    and access.status = 'revoked';
  if not found then raise exception 'Revoked managed Trusted Device access was not found'; end if;
end;
$$;

create function public.current_household_intelligence_access(
  requested_device_public_key text default null
)
returns table (
  household_id uuid,
  plan_code text,
  entitlement_state text,
  entitlement_source text,
  max_budget_usd numeric,
  valid_until timestamptz,
  device_state text,
  credential_expires_at timestamptz
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
      when entitlement.source = 'complimentary' and entitlement.status = 'active'
        and entitlement.valid_until > now() then 'entitled'
      when subscription.status = 'checkout_pending' then 'checkout_pending'
      when subscription.status = 'past_due' then 'payment_problem'
      when subscription.status in ('paused', 'canceled') then 'ended'
      when entitlement.status = 'active' and entitlement.valid_until > now() then 'entitled'
      when entitlement.household_id is not null then 'ended'
      else 'free'
    end,
    entitlement.source,
    entitlement.max_budget_usd,
    entitlement.valid_until,
    case
      when entitlement.household_id is null
        or entitlement.status <> 'active'
        or entitlement.valid_until <= now() then 'not_applicable'
      when requested_device_public_key is null then 'pending'
      when device.status = 'revoked' then 'revoked'
      when access.status = 'ready' and access.credential_expires_at > now() then 'ready'
      else 'pending'
    end,
    access.credential_expires_at
  from public.household_plans as plan
  join public.household_memberships as membership on membership.household_id = plan.household_id
  left join public.managed_intelligence_entitlements as entitlement on entitlement.household_id = plan.household_id
  left join public.billing_subscriptions as subscription on subscription.household_id = plan.household_id
  left join public.trusted_devices as device
    on device.household_id = plan.household_id and device.public_key = requested_device_public_key
  left join public.managed_intelligence_device_access as access on access.device_id = device.id
  where membership.account_id = private.current_luna_account_id()
$$;

revoke all on function public.grant_complimentary_managed_intelligence(uuid, numeric, timestamptz) from public;
revoke all on function public.apply_paddle_subscription_event(text, text, timestamptz, uuid, text, text, text, timestamptz, numeric) from public;
revoke all on function public.authorize_managed_intelligence_device_provisioning(text, uuid, text, text) from public;
revoke all on function public.record_managed_intelligence_device_access(uuid, uuid, text, text, timestamptz) from public;
revoke all on function public.current_household_intelligence_access(text) from public;
grant execute on function public.grant_complimentary_managed_intelligence(uuid, numeric, timestamptz) to service_role;
grant execute on function public.apply_paddle_subscription_event(text, text, timestamptz, uuid, text, text, text, timestamptz, numeric) to service_role;
grant execute on function public.authorize_managed_intelligence_device_provisioning(text, uuid, text, text) to authenticated;
grant execute on function public.record_managed_intelligence_device_access(uuid, uuid, text, text, timestamptz) to service_role;
grant execute on function public.current_household_intelligence_access(text) to authenticated;
