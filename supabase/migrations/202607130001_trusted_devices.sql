create extension if not exists pgsodium;

create or replace function private.authorization_field(value text)
returns text
language sql
immutable
set search_path = ''
as $$
  select pg_catalog.octet_length(value)::text || ':' || value
$$;

create table public.household_key_epochs (
  household_id uuid primary key references public.households(id) on delete cascade,
  key_epoch integer not null check (key_epoch > 0),
  recovery_envelope text not null check (char_length(recovery_envelope) > 0),
  recovery_verification_key text not null check (char_length(recovery_verification_key) > 0),
  updated_at timestamptz not null default now()
);

create table public.trusted_devices (
  id uuid primary key default gen_random_uuid(),
  household_id uuid not null references public.households(id) on delete cascade,
  enrolled_by_account_id uuid not null references public.luna_accounts(id),
  label text not null check (char_length(trim(label)) between 1 and 120),
  public_key text not null check (char_length(public_key) > 0),
  key_envelope text not null check (char_length(key_envelope) > 0),
  key_epoch integer not null check (key_epoch > 0),
  status text not null check (status in ('active', 'revoked')),
  created_at timestamptz not null default now(),
  revoked_at timestamptz,
  unique (household_id, public_key)
);

alter table public.household_key_epochs enable row level security;
alter table public.trusted_devices enable row level security;
revoke all on public.household_key_epochs from anon, authenticated;
revoke all on public.trusted_devices from anon, authenticated;
grant select on public.household_key_epochs to authenticated;
grant select on public.trusted_devices to authenticated;

create policy "AAL2 Household members read encrypted recovery coordination"
  on public.household_key_epochs for select to authenticated
  using (private.is_household_member(household_id) and auth.jwt() ->> 'aal' = 'aal2');

create policy "AAL2 Household members read their Trusted Devices"
  on public.trusted_devices for select to authenticated
  using (private.is_household_member(household_id) and auth.jwt() ->> 'aal' = 'aal2');

create or replace function public.register_first_trusted_device(
  requested_label text,
  requested_public_key text,
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

  insert into public.household_key_epochs (
    household_id, key_epoch, recovery_envelope, recovery_verification_key
  ) values (
    current_household_id, 1, requested_recovery_envelope, requested_recovery_verification_key
  );
  insert into public.trusted_devices (
    household_id, enrolled_by_account_id, label, public_key, key_envelope, key_epoch, status
  ) values (
    current_household_id, current_account_id, trim(requested_label), requested_public_key,
    requested_key_envelope, 1, 'active'
  ) returning id into new_device_id;

  return query
  select device.id, device.label, device.public_key, device.key_epoch, device.status
  from public.trusted_devices as device where device.id = new_device_id;
end;
$$;

create or replace function public.current_trusted_device_recovery()
returns table (recovery_envelope text, key_epoch integer)
language sql
stable
security definer
set search_path = ''
as $$
  select epoch.recovery_envelope, epoch.key_epoch
  from public.household_key_epochs as epoch
  join public.household_memberships as membership on membership.household_id = epoch.household_id
  where membership.account_id = private.current_luna_account_id()
    and auth.jwt() ->> 'aal' = 'aal2'
$$;

create or replace function public.register_recovered_trusted_device(
  requested_label text,
  requested_public_key text,
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
  where epoch.household_id = current_household_id;
  if current_key_epoch is null or requested_key_epoch <> current_key_epoch then
    raise exception 'The Household key epoch has changed';
  end if;
  if trim(coalesce(requested_public_key, '')) = ''
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
        || private.authorization_field(requested_key_envelope),
      'UTF8'
    ),
    decode(recovery_verification_key, 'base64')
  ), false) then
    raise exception 'Recovery Key authorization is invalid';
  end if;

  insert into public.trusted_devices (
    household_id, enrolled_by_account_id, label, public_key, key_envelope, key_epoch, status
  ) values (
    current_household_id, current_account_id, trim(requested_label), requested_public_key,
    requested_key_envelope, requested_key_epoch, 'active'
  ) returning id into new_device_id;

  return query
  select device.id, device.label, device.public_key, device.key_epoch, device.status
  from public.trusted_devices as device where device.id = new_device_id;
end;
$$;

create or replace function public.current_trusted_devices()
returns table (
  device_id uuid,
  device_label text,
  device_public_key text,
  key_epoch integer,
  device_status text
)
language sql
stable
security definer
set search_path = ''
as $$
  select device.id, device.label, device.public_key, device.key_epoch, device.status
  from public.trusted_devices as device
  join public.household_memberships as membership on membership.household_id = device.household_id
  where membership.account_id = private.current_luna_account_id()
    and auth.jwt() ->> 'aal' = 'aal2'
  order by device.created_at
$$;

create or replace function public.current_trusted_device_key(requested_public_key text)
returns table (key_envelope text, key_epoch integer, device_status text)
language sql
stable
security definer
set search_path = ''
as $$
  select device.key_envelope, device.key_epoch, device.status
  from public.trusted_devices as device
  join public.household_memberships as membership on membership.household_id = device.household_id
  where membership.account_id = private.current_luna_account_id()
    and device.public_key = requested_public_key
    and auth.jwt() ->> 'aal' = 'aal2'
$$;

create or replace function public.revoke_trusted_device(
  requested_device_id uuid,
  requested_current_device_public_key text,
  requested_current_key_epoch integer,
  requested_recovery_envelope text,
  requested_device_envelopes jsonb,
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
  current_epoch integer;
  retained_count integer;
  envelope_count integer;
  distinct_envelope_count integer;
  recovery_verification_key text;
begin
  if auth.jwt() ->> 'aal' <> 'aal2' then
    raise exception 'Authenticator verification is required';
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
    into current_epoch, recovery_verification_key
  from public.household_key_epochs as epoch
  where epoch.household_id = current_household_id
  for update;
  if current_epoch is null or current_epoch <> requested_current_key_epoch then
    raise exception 'The Household key epoch has changed';
  end if;
  if not exists (
    select 1 from public.trusted_devices as device
    where device.id = requested_device_id
      and device.household_id = current_household_id
      and device.status = 'active'
  ) then
    raise exception 'The Trusted Device is not active';
  end if;
  if not exists (
    select 1 from public.trusted_devices as device
    where device.household_id = current_household_id
      and device.public_key = requested_current_device_public_key
      and device.id <> requested_device_id
      and device.status = 'active'
  ) then
    raise exception 'The current Trusted Device must remain active';
  end if;
  if jsonb_typeof(requested_device_envelopes) <> 'array'
    or trim(coalesce(requested_recovery_envelope, '')) = '' then
    raise exception 'Rotated key envelopes are required';
  end if;

  select count(*) into retained_count
  from public.trusted_devices as device
  where device.household_id = current_household_id
    and device.id <> requested_device_id
    and device.status = 'active';
  select count(*), count(distinct envelope.value ->> 'devicePublicKey')
    into envelope_count, distinct_envelope_count
  from jsonb_array_elements(requested_device_envelopes) as envelope(value)
  where trim(coalesce(envelope.value ->> 'devicePublicKey', '')) <> ''
    and trim(coalesce(envelope.value ->> 'keyEnvelope', '')) <> '';
  if envelope_count <> retained_count or distinct_envelope_count <> retained_count then
    raise exception 'Every retained Trusted Device requires one rotated key envelope';
  end if;
  if exists (
    select 1
    from public.trusted_devices as device
    left join jsonb_to_recordset(requested_device_envelopes)
      as envelope("devicePublicKey" text, "keyEnvelope" text)
      on envelope."devicePublicKey" = device.public_key
    where device.household_id = current_household_id
      and device.id <> requested_device_id
      and device.status = 'active'
      and envelope."devicePublicKey" is null
  ) or exists (
    select 1
    from jsonb_to_recordset(requested_device_envelopes)
      as envelope("devicePublicKey" text, "keyEnvelope" text)
    where not exists (
      select 1 from public.trusted_devices as device
      where device.household_id = current_household_id
        and device.id <> requested_device_id
        and device.status = 'active'
        and device.public_key = envelope."devicePublicKey"
    )
  ) then
    raise exception 'A rotated key envelope does not match a retained Trusted Device';
  end if;
  if trim(coalesce(requested_recovery_authorization_signature, '')) = '' then
    raise exception 'Recovery Key authorization is required';
  end if;
  if not coalesce(pgsodium.crypto_sign_verify_detached(
    decode(requested_recovery_authorization_signature, 'base64'),
    convert_to(
      'luna:revoke-device:v2:'
        || private.authorization_field(current_household_id::text)
        || private.authorization_field(requested_current_key_epoch::text)
        || private.authorization_field(requested_device_id::text)
        || private.authorization_field(requested_current_device_public_key)
        || private.authorization_field(requested_recovery_envelope)
        || private.authorization_field(envelope_count::text)
        || coalesce((
          select string_agg(
            private.authorization_field(envelope."devicePublicKey")
              || private.authorization_field(envelope."keyEnvelope"),
            '' order by envelope."devicePublicKey"
          )
          from jsonb_to_recordset(requested_device_envelopes)
            as envelope("devicePublicKey" text, "keyEnvelope" text)
        ), ''),
      'UTF8'
    ),
    decode(recovery_verification_key, 'base64')
  ), false) then
    raise exception 'Recovery Key authorization is invalid';
  end if;

  update public.trusted_devices as device
  set key_envelope = envelope."keyEnvelope", key_epoch = current_epoch + 1
  from jsonb_to_recordset(requested_device_envelopes)
    as envelope("devicePublicKey" text, "keyEnvelope" text)
  where device.household_id = current_household_id
    and device.public_key = envelope."devicePublicKey"
    and device.status = 'active';
  update public.trusted_devices as device
  set status = 'revoked', revoked_at = now()
  where device.id = requested_device_id;
  update public.household_key_epochs as epoch
  set key_epoch = current_epoch + 1,
      recovery_envelope = requested_recovery_envelope,
      updated_at = now()
  where epoch.household_id = current_household_id;

  return query
  select device.id, device.label, device.public_key, device.key_epoch, device.status
  from public.trusted_devices as device
  where device.household_id = current_household_id
  order by device.created_at;
end;
$$;

revoke all on function public.register_first_trusted_device(text, text, text, text, text) from public;
revoke all on function public.current_trusted_device_recovery() from public;
revoke all on function public.register_recovered_trusted_device(text, text, text, integer, text) from public;
revoke all on function public.current_trusted_devices() from public;
revoke all on function public.current_trusted_device_key(text) from public;
revoke all on function public.revoke_trusted_device(uuid, text, integer, text, jsonb, text) from public;
grant execute on function public.register_first_trusted_device(text, text, text, text, text) to authenticated;
grant execute on function public.current_trusted_device_recovery() to authenticated;
grant execute on function public.register_recovered_trusted_device(text, text, text, integer, text) to authenticated;
grant execute on function public.current_trusted_devices() to authenticated;
grant execute on function public.current_trusted_device_key(text) to authenticated;
grant execute on function public.revoke_trusted_device(uuid, text, integer, text, jsonb, text) to authenticated;
