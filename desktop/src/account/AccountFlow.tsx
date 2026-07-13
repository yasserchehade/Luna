import { FormEvent, type PropsWithChildren, useRef, useState } from "react";
import type { AccountService, HouseholdSession } from "./accountService";

type AccountFlowProps = {
  accountService: AccountService;
  initialStep?: "registration" | "signIn";
  onAuthenticated: (session: HouseholdSession) => void | Promise<void>;
};

type AccountStep =
  | { kind: "registration" }
  | { kind: "verification"; email: string }
  | { kind: "household" }
  | { kind: "recoveryRequest" }
  | { kind: "recoveryVerification"; email: string }
  | { kind: "authenticatorChallenge"; session: HouseholdSession }
  | { kind: "signIn" };

export function AccountFlow({
  accountService,
  initialStep = "registration",
  onAuthenticated,
}: AccountFlowProps) {
  const [step, setStep] = useState<AccountStep>({ kind: initialStep });
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const submissionPending = useRef(false);
  const [isSubmitting, setIsSubmitting] = useState(false);

  const submit = (
    action: (form: FormData) => Promise<void>,
    failureMessage: string,
  ) => async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (submissionPending.current) return;
    submissionPending.current = true;
    setIsSubmitting(true);
    setError("");
    try {
      await action(new FormData(event.currentTarget));
    } catch {
      setError(failureMessage);
    } finally {
      submissionPending.current = false;
      setIsSubmitting(false);
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
    await onAuthenticated(await accountService.createHousehold(String(form.get("householdName"))));
  }, "We could not create your Household. Check the name and try again.");

  const signIn = submit(async (form) => {
    const session = await accountService.signIn(
      String(form.get("email")),
      String(form.get("password")),
    );
    if (await accountService.getAuthenticatorStatus() === "challengeRequired") {
      setStep({ kind: "authenticatorChallenge", session });
      return;
    }
    await onAuthenticated(session);
  }, "The email or password is incorrect. Check your details and try again.");

  const verifyAuthenticatorChallenge = submit(async (form) => {
    if (step.kind !== "authenticatorChallenge") return;
    await accountService.verifyAuthenticatorChallenge(String(form.get("code")));
    await onAuthenticated(step.session);
  }, "That authenticator code did not work. Check the current code and try again.");

  const requestPasswordReset = submit(async (form) => {
    const email = String(form.get("email"));
    await accountService.requestPasswordReset(email);
    setStep({ kind: "recoveryVerification", email });
  }, "We could not send a recovery code. Check your connection and try again.");

  const resetPassword = submit(async (form) => {
    if (step.kind !== "recoveryVerification") return;
    await accountService.resetPassword({
      email: step.email,
      recoveryCode: String(form.get("code")),
      authenticatorCode: String(form.get("authenticatorCode")),
      newPassword: String(form.get("password")),
    });
    await accountService.signOut();
    setNotice("Your password has been changed. Sign in with your new password.");
    setStep({ kind: "signIn" });
  }, "We could not change your password. Check the code and password, then try again.");

  if (step.kind === "registration") {
    return <AccountCard key="registration" eyebrow="Welcome" title="Create your Luna account" description="Your account coordinates your Household and trusted devices. It cannot open your cabinet by itself.">
      <form onSubmit={register}>
        <label htmlFor="organiser-name">Your name</label>
        <input id="organiser-name" name="organiserName" autoComplete="name" required />
        <label htmlFor="account-email">Email address</label>
        <input id="account-email" name="email" type="email" autoComplete="email" required />
        <label htmlFor="account-password">Password</label>
        <input id="account-password" name="password" type="password" autoComplete="new-password" minLength={12} required />
        <AccountError message={error} />
        <button type="submit" disabled={isSubmitting}>Create account</button>
      </form>
      <div className="account-switch">Already have an account? <button type="button" onClick={() => setStep({ kind: "signIn" })}>Sign in</button></div>
    </AccountCard>;
  }

  if (step.kind === "verification") {
    return <AccountCard key="verification" eyebrow="Account verification" title="Check your email" description={`We sent a verification code to ${step.email}.`}>
      <form onSubmit={verify}>
        <label htmlFor="verification-code">Verification code</label>
        <input id="verification-code" name="code" inputMode="numeric" autoComplete="one-time-code" required />
        <AccountError message={error} />
        <button type="submit" disabled={isSubmitting}>Verify email</button>
      </form>
    </AccountCard>;
  }

  if (step.kind === "household") {
    return <AccountCard key="household" eyebrow="Account verified" title="Create your Household" description="Your Household employs Luna and will hold its members, shared context and permissions.">
      <form onSubmit={createHousehold}>
        <label htmlFor="household-name">Household name</label>
        <input id="household-name" name="householdName" required />
        <AccountError message={error} />
        <button type="submit" disabled={isSubmitting}>Create Household</button>
      </form>
    </AccountCard>;
  }

  if (step.kind === "recoveryRequest") {
    return <AccountCard key="recovery-request" eyebrow="Account recovery" title="Reset your password" description="Enter your email and Luna will send a recovery code if an account can use it.">
      <form onSubmit={requestPasswordReset}>
        <label htmlFor="recovery-email">Email address</label>
        <input id="recovery-email" name="email" type="email" autoComplete="email" required />
        <AccountError message={error} />
        <button type="submit" disabled={isSubmitting}>Send recovery code</button>
      </form>
      <div className="account-switch"><button type="button" onClick={() => setStep({ kind: "signIn" })}>Back to sign in</button></div>
    </AccountCard>;
  }

  if (step.kind === "recoveryVerification") {
    return <AccountCard key="recovery-verification" eyebrow="Account recovery" title="Check your email" description={`We sent a recovery code to ${step.email}.`}>
      <form onSubmit={resetPassword}>
        <label htmlFor="recovery-code">Recovery code</label>
        <input id="recovery-code" name="code" inputMode="numeric" autoComplete="one-time-code" required />
        <label htmlFor="replacement-password">New password</label>
        <input id="replacement-password" name="password" type="password" autoComplete="new-password" minLength={12} required />
        <label htmlFor="recovery-authenticator-code">Authenticator code</label>
        <input id="recovery-authenticator-code" name="authenticatorCode" inputMode="numeric" autoComplete="one-time-code" required />
        <AccountError message={error} />
        <button type="submit" disabled={isSubmitting}>Set new password</button>
      </form>
    </AccountCard>;
  }

  if (step.kind === "authenticatorChallenge") {
    return <AccountCard key="authenticator-challenge" eyebrow="Account security" title="Verify your identity" description="Enter the current code from your authenticator app before Luna opens your Household.">
      <form onSubmit={verifyAuthenticatorChallenge}>
        <label htmlFor="sign-in-authenticator-code">Authenticator code</label>
        <input id="sign-in-authenticator-code" name="code" inputMode="numeric" autoComplete="one-time-code" required />
        <AccountError message={error} />
        <button type="submit" disabled={isSubmitting}>Continue to Luna</button>
      </form>
    </AccountCard>;
  }

  return <AccountCard key="sign-in" eyebrow="Welcome back" title="Sign in to Luna" description="Sign in to return to your Household.">
    <form onSubmit={signIn}>
      <label htmlFor="sign-in-email">Email address</label>
      <input id="sign-in-email" name="email" type="email" autoComplete="email" required />
      <label htmlFor="sign-in-password">Password</label>
      <input id="sign-in-password" name="password" type="password" autoComplete="current-password" required />
      {notice && <p role="status">{notice}</p>}
      <AccountError message={error} />
      <button type="submit" disabled={isSubmitting}>Sign in</button>
    </form>
    <div className="account-switch"><button type="button" onClick={() => { setError(""); setNotice(""); setStep({ kind: "recoveryRequest" }); }}>Forgot password?</button></div>
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
