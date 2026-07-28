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
      when entitlement.source = 'complimentary'
        and entitlement.status = 'active'
        and entitlement.valid_until > now()
        and entitlement.requests_used < entitlement.request_limit
        and exists (
          select 1 from public.managed_intelligence_device_access as access
          where access.household_id = plan.household_id and access.status = 'ready'
        ) then 'ready'
      when entitlement.source = 'complimentary'
        and entitlement.status = 'active'
        and entitlement.valid_until > now()
        and entitlement.requests_used < entitlement.request_limit then 'provisioning'
      when subscription.status = 'checkout_pending' then 'checkout_pending'
      when subscription.status = 'past_due' then 'payment_problem'
      when subscription.status in ('paused', 'canceled') then 'ended'
      when entitlement.status = 'active'
        and entitlement.valid_until > now()
        and entitlement.requests_used < entitlement.request_limit
        and exists (
          select 1 from public.managed_intelligence_device_access as access
          where access.household_id = plan.household_id and access.status = 'ready'
        ) then 'ready'
      when entitlement.status = 'active'
        and entitlement.valid_until > now()
        and entitlement.requests_used < entitlement.request_limit then 'provisioning'
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
