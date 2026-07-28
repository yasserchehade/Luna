alter table public.managed_intelligence_device_access
  add column gateway_revoked_at timestamptz;

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
    on conflict (device_id) do nothing;
  else
    update public.managed_intelligence_device_access
    set status = 'revoked', updated_at = now()
    where household_id = new.household_id and status <> 'revoked';
  end if;
  return new;
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
      and entitlement.status = 'active'
      and entitlement.valid_until > now()
      and entitlement.requests_used < entitlement.request_limit
  ) then
    insert into public.managed_intelligence_device_access (device_id, household_id, status)
    values (new.id, new.household_id, 'pending')
    on conflict (device_id) do nothing;
  elsif new.status = 'revoked' then
    update public.managed_intelligence_device_access
    set status = 'revoked', updated_at = now()
    where device_id = new.id and status <> 'revoked';
  end if;
  return new;
end;
$$;

create or replace function public.revoke_complimentary_managed_intelligence(
  requested_household_id uuid
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
  update public.managed_intelligence_entitlements
  set status = 'revoked', updated_at = now()
  where household_id = requested_household_id and source = 'complimentary';
  if not found then raise exception 'A complimentary entitlement was not found'; end if;
  update public.household_plans
  set plan_code = 'free', updated_at = now()
  where household_id = requested_household_id;
end;
$$;

create or replace function public.pending_managed_intelligence_revocations()
returns table (household_id uuid, device_id uuid, gateway_key_alias text)
language plpgsql
security definer
set search_path = ''
as $$
begin
  if auth.role() <> 'service_role' then
    raise exception 'Operator authority is required';
  end if;
  update public.managed_intelligence_entitlements
  set status = 'expired', updated_at = now()
  where status = 'active' and valid_until <= now();

  return query
  select access.household_id, access.device_id, access.gateway_key_alias
  from public.managed_intelligence_device_access as access
  where access.status = 'revoked'
    and access.gateway_key_alias is not null
    and access.gateway_revoked_at is null
  order by access.updated_at;
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
  if auth.role() <> 'service_role' then
    raise exception 'Operator authority is required';
  end if;
  update public.managed_intelligence_device_access
  set gateway_key_alias = null, gateway_revoked_at = now(), updated_at = now()
  where household_id = requested_household_id
    and device_id = requested_device_id
    and status = 'revoked';
  if not found then raise exception 'Revoked managed Trusted Device access was not found'; end if;
end;
$$;

revoke all on function public.revoke_complimentary_managed_intelligence(uuid) from public;
revoke all on function public.pending_managed_intelligence_revocations() from public;
revoke all on function public.record_managed_intelligence_gateway_revoked(uuid, uuid) from public;
grant execute on function public.revoke_complimentary_managed_intelligence(uuid) to service_role;
grant execute on function public.pending_managed_intelligence_revocations() to service_role;
grant execute on function public.record_managed_intelligence_gateway_revoked(uuid, uuid) to service_role;
