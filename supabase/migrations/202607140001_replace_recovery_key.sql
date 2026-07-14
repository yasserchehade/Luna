alter table public.trusted_devices
  add column if not exists authorization_public_key text;

alter table public.trusted_devices
  add constraint trusted_devices_authorization_public_key_present
  check (authorization_public_key is null or char_length(authorization_public_key) > 0);

drop function public.register_first_trusted_device(text, text, text, text, text);
drop function public.register_recovered_trusted_device(text, text, text, integer, text);
drop function public.current_trusted_device_recovery();

create or replace function public.register_first_trusted_device(
  requested_label text,
  requested_public_key text,
  requested_authorization_public_key text,
  requested_key_envelope text,
  requested_recovery_envelope text,
  requested_recovery_verification_key text
)
returns table (
  device_id uuid,
  device_label text,
  device_public_key text,
  key_epoch integer,
  device_status text
)
language plpgsql
security definer
set search_path = ''
as $$
declare
  current_account_id uuid;
  current_household_id uuid;
  new_device_id uuid;
begin
  if auth.jwt() ->> 'aal' <> 'aal2' then
    raise exception 'Authenticator verification is required';
  end if;
  current_account_id := private.current_luna_account_id();
  select membership.household_id into current_household_id
  from public.household_memberships as membership
  where membership.account_id = current_account_id;
  if current_household_id is null then
    raise exception 'Household membership is required';
  end if;
  if exists (select 1 from public.household_key_epochs where household_id = current_household_id) then
    raise exception 'This Household already has a Trusted Device';
  end if;
  if trim(coalesce(requested_authorization_public_key, '')) = '' then
    raise exception 'Trusted Device authorization key is required';
  end if;

  insert into public.household_key_epochs (
    household_id, key_epoch, recovery_envelope, recovery_verification_key
  ) values (
    current_household_id, 1, requested_recovery_envelope, requested_recovery_verification_key
  );
  insert into public.trusted_devices (
    household_id, enrolled_by_account_id, label, public_key, authorization_public_key,
    key_envelope, key_epoch, status
  ) values (
    current_household_id, current_account_id, trim(requested_label), requested_public_key,
    requested_authorization_public_key, requested_key_envelope, 1, 'active'
  ) returning id into new_device_id;

  return query
  select device.id, device.label, device.public_key, device.key_epoch, device.status
  from public.trusted_devices as device where device.id = new_device_id;
end;
$$;

create or replace function public.current_trusted_device_recovery()
returns table (recovery_envelope text, recovery_verification_key text, key_epoch integer)
language sql
stable
security definer
set search_path = ''
as $$
  select epoch.recovery_envelope, epoch.recovery_verification_key, epoch.key_epoch
  from public.household_key_epochs as epoch
  join public.household_memberships as membership on membership.household_id = epoch.household_id
  where membership.account_id = private.current_luna_account_id()
    and auth.jwt() ->> 'aal' = 'aal2'
$$;

create or replace function public.register_recovered_trusted_device(
  requested_label text,
  requested_public_key text,
  requested_authorization_public_key text,
  requested_key_envelope text,
  requested_key_epoch integer,
  requested_recovery_authorization_signature text
)
returns table (
  device_id uuid,
  device_label text,
  device_public_key text,
  key_epoch integer,
  device_status text
)
language plpgsql
security definer
set search_path = ''
as $$
declare
  current_account_id uuid;
  current_household_id uuid;
  current_key_epoch integer;
  recovery_verification_key text;
  new_device_id uuid;
begin
  if auth.jwt() ->> 'aal' <> 'aal2' then
    raise exception 'Authenticator verification is required';
  end if;
  current_account_id := private.current_luna_account_id();
  select membership.household_id into current_household_id
  from public.household_memberships as membership
  where membership.account_id = current_account_id;
  if current_household_id is null then
    raise exception 'Household membership is required';
  end if;
  select epoch.key_epoch, epoch.recovery_verification_key
    into current_key_epoch, recovery_verification_key
  from public.household_key_epochs as epoch
  where epoch.household_id = current_household_id
  for update;
  if current_key_epoch is null or requested_key_epoch <> current_key_epoch then
    raise exception 'The Household key epoch has changed';
  end if;
  if trim(coalesce(requested_public_key, '')) = ''
    or trim(coalesce(requested_authorization_public_key, '')) = ''
    or trim(coalesce(requested_key_envelope, '')) = '' then
    raise exception 'Recovered Trusted Device key material is required';
  end if;
  if trim(coalesce(requested_recovery_authorization_signature, '')) = '' then
    raise exception 'Recovery Key authorization is required';
  end if;
  if not coalesce(pgsodium.crypto_sign_verify_detached(
    decode(requested_recovery_authorization_signature, 'base64'),
    convert_to(
      'luna:recover-device:v2:'
        || private.authorization_field(current_household_id::text)
        || private.authorization_field(requested_key_epoch::text)
        || private.authorization_field(requested_public_key)
        || private.authorization_field(requested_authorization_public_key)
        || private.authorization_field(requested_key_envelope),
      'UTF8'
    ),
    decode(recovery_verification_key, 'base64')
  ), false) then
    raise exception 'Recovery Key authorization is invalid';
  end if;

  insert into public.trusted_devices (
    household_id, enrolled_by_account_id, label, public_key, authorization_public_key,
    key_envelope, key_epoch, status
  ) values (
    current_household_id, current_account_id, trim(requested_label), requested_public_key,
    requested_authorization_public_key, requested_key_envelope, requested_key_epoch, 'active'
  ) returning id into new_device_id;

  return query
  select device.id, device.label, device.public_key, device.key_epoch, device.status
  from public.trusted_devices as device where device.id = new_device_id;
end;
$$;

create or replace function public.replace_recovery_key(
  requested_current_device_public_key text,
  requested_current_key_epoch integer,
  requested_current_recovery_verification_key text,
  requested_recovery_envelope text,
  requested_recovery_verification_key text,
  requested_device_authorization_signature text
)
returns void
language plpgsql
security definer
set search_path = ''
as $$
declare
  current_account_id uuid;
  current_household_id uuid;
  current_epoch integer;
  current_recovery_verification_key text;
  device_authorization_public_key text;
begin
  if auth.jwt() ->> 'aal' <> 'aal2' then
    raise exception 'Authenticator verification is required';
  end if;
  if not exists (
    select 1
    from jsonb_array_elements(coalesce(auth.jwt() -> 'amr', '[]'::jsonb)) as method
    where method ->> 'method' = 'totp'
      and to_timestamp((method ->> 'timestamp')::double precision) >= now() - interval '5 minutes'
  ) then
    raise exception 'Fresh authenticator verification is required';
  end if;
  current_account_id := private.current_luna_account_id();
  select membership.household_id into current_household_id
  from public.household_memberships as membership
  where membership.account_id = current_account_id
    and membership.role = 'household_organiser';
  if current_household_id is null then
    raise exception 'Household Organiser authority is required';
  end if;

  select epoch.key_epoch, epoch.recovery_verification_key
    into current_epoch, current_recovery_verification_key
  from public.household_key_epochs as epoch
  where epoch.household_id = current_household_id
  for update;
  if current_epoch is null or current_epoch <> requested_current_key_epoch then
    raise exception 'The Household key epoch has changed';
  end if;
  if current_recovery_verification_key <> requested_current_recovery_verification_key then
    raise exception 'The Recovery Key has already changed';
  end if;
  select device.authorization_public_key into device_authorization_public_key
  from public.trusted_devices as device
  where device.household_id = current_household_id
    and device.public_key = requested_current_device_public_key
    and device.status = 'active';
  if trim(coalesce(device_authorization_public_key, '')) = '' then
    raise exception 'This beta Trusted Device must be re-enrolled before replacing its Recovery Key';
  end if;
  if trim(coalesce(requested_recovery_envelope, '')) = ''
    or trim(coalesce(requested_recovery_verification_key, '')) = ''
    or trim(coalesce(requested_device_authorization_signature, '')) = '' then
    raise exception 'Replacement recovery material and Trusted Device authorization are required';
  end if;
  if not coalesce(pgsodium.crypto_sign_verify_detached(
    decode(requested_device_authorization_signature, 'base64'),
    convert_to(
      'luna:replace-recovery-key:v1:'
        || private.authorization_field(current_household_id::text)
        || private.authorization_field(requested_current_key_epoch::text)
        || private.authorization_field(requested_current_device_public_key)
        || private.authorization_field(requested_current_recovery_verification_key)
        || private.authorization_field(requested_recovery_envelope)
        || private.authorization_field(requested_recovery_verification_key),
      'UTF8'
    ),
    decode(device_authorization_public_key, 'base64')
  ), false) then
    raise exception 'Trusted Device authorization is invalid';
  end if;

  update public.household_key_epochs as epoch
  set recovery_envelope = requested_recovery_envelope,
      recovery_verification_key = requested_recovery_verification_key,
      updated_at = now()
  where epoch.household_id = current_household_id;
end;
$$;

revoke all on function public.register_first_trusted_device(text, text, text, text, text, text) from public;
revoke all on function public.register_recovered_trusted_device(text, text, text, text, integer, text) from public;
revoke all on function public.current_trusted_device_recovery() from public;
revoke all on function public.replace_recovery_key(text, integer, text, text, text, text) from public;
grant execute on function public.register_first_trusted_device(text, text, text, text, text, text) to authenticated;
grant execute on function public.register_recovered_trusted_device(text, text, text, text, integer, text) to authenticated;
grant execute on function public.current_trusted_device_recovery() to authenticated;
grant execute on function public.replace_recovery_key(text, integer, text, text, text, text) to authenticated;
