create function public.reserve_managed_intelligence_device_gateway_alias(
  requested_household_id uuid,
  requested_device_id uuid,
  requested_gateway_key_alias text
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
      updated_at = now()
  from public.trusted_devices as device
  where access.household_id = requested_household_id
    and access.device_id = requested_device_id
    and access.status = 'pending'
    and device.id = access.device_id
    and device.status = 'active'
    and exists (
      select 1 from public.managed_intelligence_entitlements as entitlement
      where entitlement.household_id = requested_household_id
        and entitlement.status = 'active'
        and entitlement.valid_until > now()
    );
  if not found then raise exception 'Managed Trusted Device access is not eligible'; end if;
end;
$$;

revoke all on function public.reserve_managed_intelligence_device_gateway_alias(uuid, uuid, text) from public;
grant execute on function public.reserve_managed_intelligence_device_gateway_alias(uuid, uuid, text) to service_role;
