create table public.household_plans (
  household_id uuid primary key references public.households(id) on delete cascade,
  plan_code text not null default 'free' check (plan_code in ('free', 'managed')),
  updated_at timestamptz not null default now()
);

create table public.managed_intelligence_entitlements (
  household_id uuid primary key references public.households(id) on delete cascade,
  source text not null check (source in ('complimentary', 'billing')),
  status text not null check (status in ('active', 'revoked', 'expired')),
  request_limit integer not null check (request_limit > 0),
  requests_used integer not null default 0 check (requests_used >= 0),
  valid_from timestamptz not null default now(),
  valid_until timestamptz not null,
  updated_at timestamptz not null default now(),
  check (valid_until > valid_from)
);

insert into public.household_plans (household_id)
select household.id from public.households as household
on conflict (household_id) do nothing;

create or replace function private.create_free_household_plan()
returns trigger
language plpgsql
security definer
set search_path = ''
as $$
begin
  insert into public.household_plans (household_id) values (new.id);
  return new;
end;
$$;

create trigger create_free_household_plan_after_household
  after insert on public.households
  for each row execute function private.create_free_household_plan();

alter table public.household_plans enable row level security;
alter table public.managed_intelligence_entitlements enable row level security;
revoke all on public.household_plans from anon, authenticated;
revoke all on public.managed_intelligence_entitlements from anon, authenticated;

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
      when entitlement.status = 'active'
        and entitlement.valid_until > now()
        and entitlement.requests_used < entitlement.request_limit
        then 'provisioning'
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
  where membership.account_id = private.current_luna_account_id()
$$;

create or replace function public.grant_complimentary_managed_intelligence(
  requested_household_id uuid,
  requested_request_limit integer,
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
  if requested_request_limit is null or requested_request_limit <= 0 then
    raise exception 'A positive request limit is required';
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
    request_limit,
    requests_used,
    valid_from,
    valid_until,
    updated_at
  ) values (
    requested_household_id,
    'complimentary',
    'active',
    requested_request_limit,
    0,
    now(),
    requested_valid_until,
    now()
  )
  on conflict (household_id) do update set
    source = excluded.source,
    status = excluded.status,
    request_limit = excluded.request_limit,
    requests_used = 0,
    valid_from = excluded.valid_from,
    valid_until = excluded.valid_until,
    updated_at = excluded.updated_at;

  update public.household_plans
  set plan_code = 'managed', updated_at = now()
  where household_id = requested_household_id;
end;
$$;

revoke all on function public.current_household_intelligence_access() from public;
revoke all on function public.grant_complimentary_managed_intelligence(uuid, integer, timestamptz) from public;
grant execute on function public.current_household_intelligence_access() to authenticated;
grant execute on function public.grant_complimentary_managed_intelligence(uuid, integer, timestamptz) to service_role;
