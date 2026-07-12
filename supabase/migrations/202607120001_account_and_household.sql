create schema if not exists private;
revoke all on schema private from public;

create table public.luna_accounts (
  id uuid primary key default gen_random_uuid(),
  display_name text not null check (char_length(trim(display_name)) between 1 and 120),
  created_at timestamptz not null default now()
);

create table public.external_identities (
  provider text not null,
  provider_subject text not null,
  account_id uuid not null references public.luna_accounts(id) on delete cascade,
  created_at timestamptz not null default now(),
  primary key (provider, provider_subject),
  unique (account_id, provider)
);

create table public.households (
  id uuid primary key default gen_random_uuid(),
  name text not null check (char_length(trim(name)) between 1 and 120),
  created_by_account_id uuid not null references public.luna_accounts(id),
  created_at timestamptz not null default now()
);

create table public.household_memberships (
  household_id uuid not null references public.households(id) on delete cascade,
  account_id uuid not null references public.luna_accounts(id) on delete cascade,
  role text not null check (role in ('household_organiser', 'adult_member', 'dependent_member')),
  created_at timestamptz not null default now(),
  primary key (household_id, account_id),
  unique (account_id)
);

create unique index one_household_organiser
  on public.household_memberships (household_id)
  where role = 'household_organiser';

create or replace function private.current_luna_account_id()
returns uuid
language sql
stable
security definer
set search_path = ''
as $$
  select identity.account_id
  from public.external_identities as identity
  where identity.provider = 'supabase'
    and identity.provider_subject = (select auth.uid())::text
$$;

create or replace function private.is_household_member(subject_household_id uuid)
returns boolean
language sql
stable
security definer
set search_path = ''
as $$
  select exists (
    select 1
    from public.household_memberships as membership
    where membership.household_id = subject_household_id
      and membership.account_id = private.current_luna_account_id()
  )
$$;

revoke all on function private.current_luna_account_id() from public;
revoke all on function private.is_household_member(uuid) from public;
grant usage on schema private to authenticated;
grant execute on function private.current_luna_account_id() to authenticated;
grant execute on function private.is_household_member(uuid) to authenticated;

create or replace function private.create_luna_account_for_identity()
returns trigger
language plpgsql
security definer
set search_path = ''
as $$
declare
  new_account_id uuid;
  requested_name text;
begin
  requested_name := trim(coalesce(new.raw_user_meta_data ->> 'organiser_name', ''));
  if requested_name = '' then
    requested_name := 'Household Organiser';
  end if;

  insert into public.luna_accounts (display_name)
  values (requested_name)
  returning id into new_account_id;

  insert into public.external_identities (provider, provider_subject, account_id)
  values ('supabase', new.id::text, new_account_id);

  return new;
end;
$$;

create trigger create_luna_account_after_signup
  after insert on auth.users
  for each row execute function private.create_luna_account_for_identity();

alter table public.luna_accounts enable row level security;
alter table public.external_identities enable row level security;
alter table public.households enable row level security;
alter table public.household_memberships enable row level security;

revoke all on public.luna_accounts from anon, authenticated;
revoke all on public.external_identities from anon, authenticated;
revoke all on public.households from anon, authenticated;
revoke all on public.household_memberships from anon, authenticated;

grant select on public.luna_accounts to authenticated;
grant select on public.external_identities to authenticated;
grant select on public.households to authenticated;
grant select on public.household_memberships to authenticated;

create policy "Account members read their own Luna Account"
  on public.luna_accounts for select to authenticated
  using (id = private.current_luna_account_id());

create policy "Account members read their own External Identity"
  on public.external_identities for select to authenticated
  using (account_id = private.current_luna_account_id());

create policy "Household members read their Household"
  on public.households for select to authenticated
  using (private.is_household_member(id));

create policy "Household members read their membership"
  on public.household_memberships for select to authenticated
  using (private.is_household_member(household_id));

create or replace function public.create_household(requested_name text)
returns table (
  account_id uuid,
  organiser_name text,
  email text,
  household_id uuid,
  household_name text
)
language plpgsql
security definer
set search_path = ''
as $$
declare
  current_account_id uuid;
  new_household_id uuid;
begin
  current_account_id := private.current_luna_account_id();
  if current_account_id is null then
    raise exception 'A verified Luna Account is required';
  end if;
  if not exists (
    select 1 from auth.users as auth_user
    where auth_user.id = (select auth.uid())
      and auth_user.email_confirmed_at is not null
  ) then
    raise exception 'A verified Luna Account is required';
  end if;
  if trim(coalesce(requested_name, '')) = '' then
    raise exception 'A Household name is required';
  end if;
  if exists (
    select 1 from public.household_memberships as membership
    where membership.account_id = current_account_id
  ) then
    raise exception 'This Luna Account already belongs to a Household';
  end if;

  insert into public.households (name, created_by_account_id)
  values (trim(requested_name), current_account_id)
  returning id into new_household_id;

  insert into public.household_memberships (household_id, account_id, role)
  values (new_household_id, current_account_id, 'household_organiser');

  return query
  select account.id, account.display_name, auth.jwt() ->> 'email', household.id, household.name
  from public.luna_accounts as account
  join public.households as household on household.id = new_household_id
  where account.id = current_account_id;
end;
$$;

create or replace function public.current_household()
returns table (
  account_id uuid,
  organiser_name text,
  email text,
  household_id uuid,
  household_name text
)
language sql
stable
security definer
set search_path = ''
as $$
  select account.id, account.display_name, auth.jwt() ->> 'email', household.id, household.name
  from public.luna_accounts as account
  join public.household_memberships as membership on membership.account_id = account.id
  join public.households as household on household.id = membership.household_id
  where account.id = private.current_luna_account_id()
$$;

revoke all on function public.create_household(text) from public;
revoke all on function public.current_household() from public;
grant execute on function public.create_household(text) to authenticated;
grant execute on function public.current_household() to authenticated;
