create or replace function private.synchronize_managed_intelligence_device_access()
returns trigger
language plpgsql
security definer
set search_path = ''
as $$
begin
  if new.status = 'active' and new.valid_until > now() and new.requests_used < new.request_limit then
    insert into public.managed_intelligence_device_access (
      device_id,
      household_id,
      status,
      gateway_key_alias,
      gateway_revoked_at,
      updated_at
    )
    select device.id, device.household_id, 'pending', null, null, now()
    from public.trusted_devices as device
    where device.household_id = new.household_id and device.status = 'active'
    on conflict (device_id) do update set
      status = case
        when public.managed_intelligence_device_access.status = 'revoked' then 'pending'
        else public.managed_intelligence_device_access.status
      end,
      gateway_key_alias = case
        when public.managed_intelligence_device_access.status = 'revoked' then null
        else public.managed_intelligence_device_access.gateway_key_alias
      end,
      gateway_revoked_at = case
        when public.managed_intelligence_device_access.status = 'revoked' then null
        else public.managed_intelligence_device_access.gateway_revoked_at
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
    insert into public.managed_intelligence_device_access (
      device_id,
      household_id,
      status,
      gateway_key_alias,
      gateway_revoked_at,
      updated_at
    ) values (new.id, new.household_id, 'pending', null, null, now())
    on conflict (device_id) do update set
      status = case
        when public.managed_intelligence_device_access.status = 'revoked' then 'pending'
        else public.managed_intelligence_device_access.status
      end,
      gateway_key_alias = case
        when public.managed_intelligence_device_access.status = 'revoked' then null
        else public.managed_intelligence_device_access.gateway_key_alias
      end,
      gateway_revoked_at = case
        when public.managed_intelligence_device_access.status = 'revoked' then null
        else public.managed_intelligence_device_access.gateway_revoked_at
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
