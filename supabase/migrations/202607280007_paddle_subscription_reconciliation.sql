create or replace function public.paddle_subscriptions_for_reconciliation()
returns table (household_id uuid, external_customer_id text, external_subscription_id text)
language plpgsql
security definer
set search_path = ''
as $$
begin
  if auth.role() <> 'service_role' then
    raise exception 'Operator authority is required';
  end if;
  return query
  select
    subscription.household_id,
    subscription.external_customer_id,
    subscription.external_subscription_id
  from public.billing_subscriptions as subscription
  where subscription.billing_provider = 'paddle'
    and subscription.external_customer_id is not null
    and subscription.external_subscription_id is not null
  order by subscription.updated_at;
end;
$$;

revoke all on function public.paddle_subscriptions_for_reconciliation() from public;
grant execute on function public.paddle_subscriptions_for_reconciliation() to service_role;
