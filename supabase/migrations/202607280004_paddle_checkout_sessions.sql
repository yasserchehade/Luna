alter table public.billing_subscriptions
  alter column external_customer_id drop not null,
  alter column external_subscription_id drop not null;

alter table public.billing_subscriptions
  add column external_transaction_id text unique
    check (external_transaction_id is null or char_length(external_transaction_id) > 0);

create or replace function public.current_household_billing_context()
returns table (
  household_id uuid,
  organiser_email text,
  external_customer_id text,
  external_subscription_id text
)
language sql
stable
security definer
set search_path = ''
as $$
  select
    membership.household_id,
    auth.jwt() ->> 'email',
    subscription.external_customer_id,
    subscription.external_subscription_id
  from public.household_memberships as membership
  left join public.billing_subscriptions as subscription
    on subscription.household_id = membership.household_id
  where membership.account_id = private.current_luna_account_id()
    and membership.role = 'household_organiser'
$$;

create or replace function public.record_paddle_checkout_pending(
  requested_household_id uuid,
  requested_transaction_id text
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
  if trim(coalesce(requested_transaction_id, '')) = '' then
    raise exception 'A Paddle transaction identifier is required';
  end if;

  insert into public.billing_subscriptions (
    household_id,
    billing_provider,
    external_transaction_id,
    external_customer_id,
    external_subscription_id,
    status,
    last_event_at,
    updated_at
  ) values (
    requested_household_id,
    'paddle',
    requested_transaction_id,
    null,
    null,
    'checkout_pending',
    '-infinity'::timestamptz,
    now()
  )
  on conflict (household_id) do update set
    external_transaction_id = excluded.external_transaction_id,
    status = 'checkout_pending',
    updated_at = excluded.updated_at
  where public.billing_subscriptions.status in ('checkout_pending', 'paused', 'canceled');

  if not found then
    raise exception 'This Household must manage its existing subscription';
  end if;
end;
$$;

revoke all on function public.current_household_billing_context() from public;
revoke all on function public.record_paddle_checkout_pending(uuid, text) from public;
grant execute on function public.current_household_billing_context() to authenticated;
grant execute on function public.record_paddle_checkout_pending(uuid, text) to service_role;
