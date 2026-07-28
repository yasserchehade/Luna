alter table public.managed_intelligence_device_access
  add column provisioning_lease_expires_at timestamptz;

create function private.rotate_managed_intelligence_budget_scope()
returns trigger
language plpgsql
set search_path = ''
as $$
begin
  if old.max_budget_usd is distinct from new.max_budget_usd
    and old.budget_scope_id is not distinct from new.budget_scope_id then
    new.budget_scope_id = gen_random_uuid();
  end if;
  return new;
end;
$$;

create trigger rotate_managed_intelligence_budget_scope_before_cap_change
before update of max_budget_usd on public.managed_intelligence_entitlements
for each row execute function private.rotate_managed_intelligence_budget_scope();

create function public.reserve_managed_intelligence_device_gateway_alias(
  requested_household_id uuid,
  requested_device_id uuid,
  requested_gateway_key_alias text,
  requested_budget_scope_id uuid,
  requested_max_budget_usd numeric
)
returns void
language plpgsql
security definer
set search_path = ''
as $$
begin
  if auth.role() <> 'service_role' then raise exception 'Operator authority is required'; end if;
  if requested_gateway_key_alias <> 'luna-managed-' || requested_device_id::text then
    raise exception 'The managed gateway alias is invalid';
  end if;

  update public.managed_intelligence_device_access as access
  set gateway_key_alias = requested_gateway_key_alias,
      gateway_revoked_at = null,
      provisioning_lease_expires_at = now() + interval '2 minutes',
      updated_at = now()
  from public.trusted_devices as device,
       public.managed_intelligence_entitlements as entitlement
  where access.household_id = requested_household_id
    and access.device_id = requested_device_id
    and access.status = 'pending'
    and device.id = access.device_id
    and device.status = 'active'
    and entitlement.household_id = requested_household_id
    and entitlement.status = 'active'
    and entitlement.valid_until > now()
    and entitlement.budget_scope_id = requested_budget_scope_id
    and entitlement.max_budget_usd = requested_max_budget_usd;
  if not found then raise exception 'Managed Trusted Device access is not eligible'; end if;
end;
$$;

create or replace function public.pending_managed_intelligence_revocations()
returns table (household_id uuid, device_id uuid, gateway_key_alias text)
language plpgsql
security definer
set search_path = ''
as $$
begin
  if auth.role() <> 'service_role' then raise exception 'Operator authority is required'; end if;
  update public.managed_intelligence_entitlements
  set status = 'expired', updated_at = now()
  where status = 'active' and valid_until <= now();

  return query
  select access.household_id, access.device_id, access.gateway_key_alias
  from public.managed_intelligence_device_access as access
  where access.status = 'revoked'
    and access.gateway_key_alias is not null
    and access.gateway_revoked_at is null
    and (
      access.provisioning_lease_expires_at is null
      or access.provisioning_lease_expires_at <= now()
    )
  order by access.updated_at;
end;
$$;

create or replace function public.record_managed_intelligence_device_access(
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
      provisioning_lease_expires_at = case when requested_status = 'ready' then null else access.provisioning_lease_expires_at end,
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
      provisioning_lease_expires_at = null,
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

revoke all on function public.reserve_managed_intelligence_device_gateway_alias(uuid, uuid, text, uuid, numeric) from public;
grant execute on function public.reserve_managed_intelligence_device_gateway_alias(uuid, uuid, text, uuid, numeric) to service_role;
