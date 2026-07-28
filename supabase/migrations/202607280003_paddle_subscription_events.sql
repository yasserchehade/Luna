create table public.billing_subscriptions (
  household_id uuid primary key references public.households(id) on delete cascade,
  billing_provider text not null check (billing_provider = 'paddle'),
  external_customer_id text not null check (char_length(external_customer_id) > 0),
  external_subscription_id text not null unique check (char_length(external_subscription_id) > 0),
  status text not null check (status in ('checkout_pending', 'trialing', 'active', 'past_due', 'paused', 'canceled')),
  last_event_at timestamptz not null,
  updated_at timestamptz not null default now()
);

create table public.billing_events (
  billing_provider text not null check (billing_provider = 'paddle'),
  external_event_id text not null check (char_length(external_event_id) > 0),
  household_id uuid not null references public.households(id) on delete cascade,
  external_subscription_id text not null check (char_length(external_subscription_id) > 0),
  event_type text not null check (event_type in ('subscription.created', 'subscription.updated', 'subscription.canceled')),
  occurred_at timestamptz not null,
  applied boolean not null default false,
  received_at timestamptz not null default now(),
  primary key (billing_provider, external_event_id)
);

alter table public.billing_subscriptions enable row level security;
alter table public.billing_events enable row level security;
revoke all on public.billing_subscriptions from anon, authenticated;
revoke all on public.billing_events from anon, authenticated;

create or replace function public.apply_paddle_subscription_event(
  requested_event_id text,
  requested_event_type text,
  requested_occurred_at timestamptz,
  requested_household_id uuid,
  requested_customer_id text,
  requested_subscription_id text,
  requested_status text,
  requested_valid_until timestamptz,
  requested_request_limit integer
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
  if auth.role() <> 'service_role' then
    raise exception 'Operator authority is required';
  end if;
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
  if requested_request_limit is null or requested_request_limit <= 0 then
    raise exception 'A positive request limit is required';
  end if;
  if requested_status in ('trialing', 'active')
    and (requested_valid_until is null or requested_valid_until <= requested_occurred_at) then
    raise exception 'An active subscription requires a future billing-period end';
  end if;

  perform pg_catalog.pg_advisory_xact_lock(
    pg_catalog.hashtextextended(requested_household_id::text, 0)
  );

  insert into public.billing_events (
    billing_provider,
    external_event_id,
    household_id,
    external_subscription_id,
    event_type,
    occurred_at
  ) values (
    'paddle',
    requested_event_id,
    requested_household_id,
    requested_subscription_id,
    requested_event_type,
    requested_occurred_at
  ) on conflict (billing_provider, external_event_id) do nothing;
  get diagnostics event_inserted_count = row_count;
  if event_inserted_count = 0 then return false; end if;

  insert into public.billing_subscriptions (
    household_id,
    billing_provider,
    external_customer_id,
    external_subscription_id,
    status,
    last_event_at,
    updated_at
  ) values (
    requested_household_id,
    'paddle',
    requested_customer_id,
    requested_subscription_id,
    requested_status,
    requested_occurred_at,
    now()
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
      'billing',
      'active',
      requested_request_limit,
      0,
      requested_occurred_at,
      requested_valid_until,
      now()
    )
    on conflict (household_id) do update set
      source = excluded.source,
      status = excluded.status,
      request_limit = excluded.request_limit,
      requests_used = case
        when public.managed_intelligence_entitlements.valid_until = excluded.valid_until
          then public.managed_intelligence_entitlements.requests_used
        else 0
      end,
      valid_from = excluded.valid_from,
      valid_until = excluded.valid_until,
      updated_at = excluded.updated_at;

    update public.household_plans
    set plan_code = 'managed', updated_at = now()
    where household_id = requested_household_id;
  else
    update public.managed_intelligence_entitlements
    set status = 'revoked', updated_at = now()
    where household_id = requested_household_id
      and source = 'billing';

    if requested_status in ('paused', 'canceled') then
      update public.household_plans
      set plan_code = 'free', updated_at = now()
      where household_id = requested_household_id;
    end if;
  end if;

  update public.billing_events
  set applied = true
  where billing_provider = 'paddle'
    and external_event_id = requested_event_id;
  return true;
end;
$$;

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
      when subscription.status = 'checkout_pending' then 'checkout_pending'
      when subscription.status = 'past_due' then 'payment_problem'
      when subscription.status in ('paused', 'canceled') then 'ended'
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
  left join public.billing_subscriptions as subscription
    on subscription.household_id = plan.household_id
  where membership.account_id = private.current_luna_account_id()
$$;

revoke all on function public.apply_paddle_subscription_event(text, text, timestamptz, uuid, text, text, text, timestamptz, integer) from public;
grant execute on function public.apply_paddle_subscription_event(text, text, timestamptz, uuid, text, text, text, timestamptz, integer) to service_role;
