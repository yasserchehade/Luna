import { FormEvent, type PropsWithChildren, useState } from "react";
import type { AccountService, HouseholdSession } from "./accountService";

type AccountFlowProps = {
  accountService: AccountService;
  initialStep?: "registration" | "signIn";
  onAuthenticated: (session: HouseholdSession) => void;
};

type AccountStep =
  | { kind: "registration" }
  | { kind: "verification"; email: string }
  | { kind: "household" }
  | { kind: "signIn" };

export function AccountFlow({
  accountService,
  initialStep = "registration",
  onAuthenticated,
}: AccountFlowProps) {
  const [step, setStep] = useState<AccountStep>({ kind: initialStep });
  const [error, setError] = useState("");

  const submit = (
    action: (form: FormData) => Promise<void>,
    failureMessage: string,
  ) => async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setError("");
    try {
      await action(new FormData(event.currentTarget));
    } catch {
      setError(failureMessage);
    }
  };

  const register = submit(async (form) => {
    const result = await accountService.register({
      organiserName: String(form.get("organiserName")),
      email: String(form.get("email")),
      password: String(form.get("password")),
    });
    setStep({ kind: "verification", email: result.email });
  }, "We could not create your account. Check your details and try again.");

  const verify = submit(async (form) => {
    if (step.kind !== "verification") return;
    await accountService.verifyEmail(step.email, String(form.get("code")));
    setStep({ kind: "household" });
  }, "That verification code did not work. Check the code and try again.");

  const createHousehold = submit(async (form) => {
    onAuthenticated(await accountService.createHousehold(String(form.get("householdName"))));
  }, "We could not create your Household. Check the name and try again.");

  const signIn = submit(async (form) => {
    onAuthenticated(await accountService.signIn(
      String(form.get("email")),
      String(form.get("password")),
    ));
  }, "The email or password is incorrect. Check your details and try again.");

  if (step.kind === "registration") {
    return <AccountCard eyebrow="Welcome" title="Create your Luna account" description="Your account coordinates your Household and trusted devices. It cannot open your cabinet by itself.">
      <form onSubmit={register}>
        <label htmlFor="organiser-name">Your name</label>
        <input id="organiser-name" name="organiserName" autoComplete="name" required />
        <label htmlFor="account-email">Email address</label>
        <input id="account-email" name="email" type="email" autoComplete="email" required />
        <label htmlFor="account-password">Password</label>
        <input id="account-password" name="password" type="password" autoComplete="new-password" minLength={12} required />
        <AccountError message={error} />
        <button type="submit">Create account</button>
      </form>
      <div className="account-switch">Already have an account? <button type="button" onClick={() => setStep({ kind: "signIn" })}>Sign in</button></div>
    </AccountCard>;
  }

  if (step.kind === "verification") {
    return <AccountCard eyebrow="Account verification" title="Check your email" description={`We sent a verification code to ${step.email}.`}>
      <form onSubmit={verify}>
        <label htmlFor="verification-code">Verification code</label>
        <input id="verification-code" name="code" inputMode="numeric" autoComplete="one-time-code" required />
        <AccountError message={error} />
        <button type="submit">Verify email</button>
      </form>
    </AccountCard>;
  }

  if (step.kind === "household") {
    return <AccountCard eyebrow="Account verified" title="Create your Household" description="Your Household employs Luna and will hold its members, shared context and permissions.">
      <form onSubmit={createHousehold}>
        <label htmlFor="household-name">Household name</label>
        <input id="household-name" name="householdName" required />
        <AccountError message={error} />
        <button type="submit">Create Household</button>
      </form>
    </AccountCard>;
  }

  return <AccountCard eyebrow="Welcome back" title="Sign in to Luna" description="Sign in to return to your Household.">
    <form onSubmit={signIn}>
      <label htmlFor="sign-in-email">Email address</label>
      <input id="sign-in-email" name="email" type="email" autoComplete="email" required />
      <label htmlFor="sign-in-password">Password</label>
      <input id="sign-in-password" name="password" type="password" autoComplete="current-password" required />
      <AccountError message={error} />
      <button type="submit">Sign in</button>
    </form>
    <div className="account-switch">New to Luna? <button type="button" onClick={() => setStep({ kind: "registration" })}>Create account</button></div>
  </AccountCard>;
}

type AccountCardProps = PropsWithChildren<{
  eyebrow: string;
  title: string;
  description: string;
}>;

function AccountCard({ eyebrow, title, description, children }: AccountCardProps) {
  return <main className="account-screen"><section className="account-card">
    <div className="account-brand"><span aria-hidden="true">L</span><strong>Luna</strong></div>
    <p className="eyebrow">{eyebrow}</p>
    <h1>{title}</h1>
    <p>{description}</p>
    {children}
  </section></main>;
}

function AccountError({ message }: { message: string }) {
  return message ? <p className="account-error" role="alert">{message}</p> : null;
}
