alter table public.trusted_devices
  add column if not exists activated_key_epoch integer,
  add column if not exists revoked_after_key_epoch integer,
  add column if not exists revoked_after_sequence bigint,
  add column if not exists revoked_after_event_digest text;

update public.trusted_devices
set activated_key_epoch = key_epoch
where activated_key_epoch is null;

alter table public.trusted_devices
  alter column activated_key_epoch set not null;

alter table public.trusted_devices
  add constraint trusted_devices_activation_epoch_positive
  check (activated_key_epoch > 0),
  add constraint trusted_devices_portable_cutoff_complete
  check (
    (
      revoked_after_key_epoch is null
      and revoked_after_sequence is null
      and revoked_after_event_digest is null
    )
    or (
      revoked_after_key_epoch > 0
      and revoked_after_sequence > 0
      and revoked_after_event_digest ~ '^[0-9a-f]{64}$'
    )
  );

create or replace function private.set_trusted_device_activation_epoch()
returns trigger
language plpgsql
security definer
set search_path = ''
as $$
begin
  if new.activated_key_epoch is null then
    new.activated_key_epoch := new.key_epoch;
  end if;
  return new;
end;
$$;

drop trigger if exists trusted_device_activation_epoch on public.trusted_devices;
create trigger trusted_device_activation_epoch
before insert on public.trusted_devices
for each row execute function private.set_trusted_device_activation_epoch();

drop function if exists public.current_trusted_devices();

create function public.current_trusted_devices()
returns table (
  device_id uuid,
  device_label text,
  device_public_key text,
  authorization_public_key text,
  activated_key_epoch integer,
  revoked_after_key_epoch integer,
  revoked_after_sequence bigint,
  revoked_after_event_digest text,
  key_epoch integer,
  device_status text
)
language sql
stable
security definer
set search_path = ''
as $$
  select
    device.id,
    device.label,
    device.public_key,
    device.authorization_public_key,
    device.activated_key_epoch,
    device.revoked_after_key_epoch,
    device.revoked_after_sequence,
    device.revoked_after_event_digest,
    device.key_epoch,
    device.status
  from public.trusted_devices as device
  join public.household_memberships as membership on membership.household_id = device.household_id
  where membership.account_id = private.current_luna_account_id()
    and auth.jwt() ->> 'aal' = 'aal2'
  order by device.created_at
$$;

revoke all on function public.current_trusted_devices() from public;
grant execute on function public.current_trusted_devices() to authenticated;

create or replace function public.revoke_trusted_device_with_portable_cutoff(
  requested_device_id uuid,
  requested_current_device_public_key text,
  requested_current_key_epoch integer,
  requested_recovery_envelope text,
  requested_device_envelopes jsonb,
  requested_recovery_authorization_signature text,
  requested_revoked_after_key_epoch integer,
  requested_revoked_after_sequence bigint,
  requested_revoked_after_event_digest text
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
begin
  if not (
    (
      requested_revoked_after_key_epoch is null
      and requested_revoked_after_sequence is null
      and requested_revoked_after_event_digest is null
    )
    or (
      requested_revoked_after_key_epoch > 0
      and requested_revoked_after_sequence > 0
      and requested_revoked_after_event_digest ~ '^[0-9a-f]{64}$'
    )
  ) then
    raise exception 'The Portable Memory revocation cutoff is invalid';
  end if;

  perform *
  from public.revoke_trusted_device(
    requested_device_id,
    requested_current_device_public_key,
    requested_current_key_epoch,
    requested_recovery_envelope,
    requested_device_envelopes,
    requested_recovery_authorization_signature
  );

  update public.trusted_devices as device
  set revoked_after_key_epoch = requested_revoked_after_key_epoch,
      revoked_after_sequence = requested_revoked_after_sequence,
      revoked_after_event_digest = requested_revoked_after_event_digest
  where device.id = requested_device_id
    and device.status = 'revoked';

  return query
  select device.id, device.label, device.public_key, device.key_epoch, device.status
  from public.trusted_devices as device
  where device.household_id = (
    select target.household_id
    from public.trusted_devices as target
    where target.id = requested_device_id
  )
  order by device.created_at;
end;
$$;

revoke all on function public.revoke_trusted_device_with_portable_cutoff(
  uuid, text, integer, text, jsonb, text, integer, bigint, text
) from public;
revoke execute on function public.revoke_trusted_device(
  uuid, text, integer, text, jsonb, text
) from authenticated;
grant execute on function public.revoke_trusted_device_with_portable_cutoff(
  uuid, text, integer, text, jsonb, text, integer, bigint, text
) to authenticated;
