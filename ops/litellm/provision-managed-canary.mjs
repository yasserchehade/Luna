import { spawnSync } from "node:child_process";
import { createHmac, randomBytes } from "node:crypto";
import {
  mkdtempSync,
  readFileSync,
  rmdirSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const scriptDirectory = path.dirname(fileURLToPath(import.meta.url));
const repositoryRoot = path.resolve(scriptDirectory, "..", "..");

const argumentsByName = parseArguments(process.argv.slice(2));
if (!argumentsByName.confirmLive) {
  fail("Refusing to change the linked Supabase project without --confirm-live.");
}

const projectRef = requiredArgument(argumentsByName, "projectRef");
const email = requiredArgument(argumentsByName, "email").toLowerCase();
const credentialPath = path.resolve(requiredArgument(argumentsByName, "credentialPath"));
const householdName = argumentsByName.householdName ?? "Luna Managed Canary";
const budgetUsd = boundedNumber(argumentsByName.budgetUsd ?? "0.25", "budget-usd", 0, 100);
const validDays = boundedInteger(argumentsByName.validDays ?? "14", "valid-days", 1, 90);
const supabaseUrl = `https://${projectRef}.supabase.co`;
if (!/^[a-z0-9]{20}$/u.test(projectRef)) fail("--project-ref is not a valid Supabase project reference.");
if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/u.test(email)) fail("--email is not a valid account email.");
if (process.platform !== "win32") fail("This canary provisioner currently requires Windows DPAPI.");
if (
  credentialPath === repositoryRoot
  || credentialPath.startsWith(`${repositoryRoot}${path.sep}`)
) {
  fail("--credential-path must be outside the public repository.");
}
assertLinkedProject(projectRef);

const apiKeys = readProjectApiKeys(projectRef);
const publishableKeyRecord = selectApiKey(apiKeys, (key) => key.type === "publishable")
  ?? selectApiKey(apiKeys, (key) => key.name === "anon");
if (!publishableKeyRecord) fail("The linked project did not return a publishable API key.");
const publishableKey = publishableKeyRecord.api_key;
const password = `${randomBytes(30).toString("base64url")}Aa1!`;
const validUntil = new Date(Date.now() + validDays * 24 * 60 * 60 * 1_000);
validUntil.setUTCSeconds(0, 0);
const household = bootstrapCanaryThroughLinkedDatabase({
  email,
  password,
  householdName,
  budgetUsd,
  validUntil: validUntil.toISOString(),
});

const signIn = await requestJson("Canary sign-in", `${supabaseUrl}/auth/v1/token?grant_type=password`, {
  method: "POST",
  headers: apiHeaders(publishableKey),
  body: JSON.stringify({ email, password }),
});
if (typeof signIn.access_token !== "string" || !signIn.access_token) {
  fail("The canary account did not receive an access token.");
}

const totp = await enrollAuthenticator(supabaseUrl, publishableKey, signIn.access_token);

const projection = await requestJson("Managed entitlement verification", `${supabaseUrl}/rest/v1/rpc/current_household_intelligence_access`, {
  method: "POST",
  headers: apiHeaders(publishableKey, totp.accessToken),
  body: JSON.stringify({ requested_device_public_key: null }),
});
const access = singleRow(projection);
if (
  access?.household_id !== household.household_id
  || access?.plan_code !== "managed"
  || access?.entitlement_state !== "entitled"
  || access?.entitlement_source !== "complimentary"
) {
  fail("The canary Household did not receive the expected complimentary managed entitlement.");
}

saveProtectedCredential(credentialPath, {
  email,
  password,
  totpSecret: totp.secret,
  projectRef,
  householdId: household.household_id,
  householdName: household.household_name,
  validUntil: validUntil.toISOString(),
});

process.stdout.write([
  "Managed canary Household ready.",
  `Account: ${email}`,
  `Household: ${household.household_name} (${household.household_id})`,
  `Entitlement: complimentary, US$${budgetUsd.toFixed(2)}, expires ${validUntil.toISOString()}`,
  `Windows-protected credentials: ${credentialPath}`,
].join("\n"));

function parseArguments(values) {
  const result = {};
  for (let index = 0; index < values.length; index += 1) {
    const value = values[index];
    if (value === "--confirm-live") {
      result.confirmLive = true;
      continue;
    }
    if (!value.startsWith("--")) fail(`Unexpected argument: ${value}`);
    const name = value.slice(2).replace(/-([a-z])/g, (_, letter) => letter.toUpperCase());
    const argumentValue = values[index + 1];
    if (!argumentValue || argumentValue.startsWith("--")) fail(`Missing value for ${value}.`);
    result[name] = argumentValue;
    index += 1;
  }
  return result;
}

function requiredArgument(values, name) {
  const value = values[name];
  if (!value) fail(`Missing required --${name.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`)}.`);
  return value;
}

function boundedNumber(value, name, minimumExclusive, maximumInclusive) {
  const number = Number(value);
  if (!Number.isFinite(number) || number <= minimumExclusive || number > maximumInclusive) {
    fail(`--${name} must be greater than ${minimumExclusive} and no more than ${maximumInclusive}.`);
  }
  return number;
}

function boundedInteger(value, name, minimumInclusive, maximumInclusive) {
  const number = Number(value);
  if (!Number.isInteger(number) || number < minimumInclusive || number > maximumInclusive) {
    fail(`--${name} must be an integer from ${minimumInclusive} through ${maximumInclusive}.`);
  }
  return number;
}

function readProjectApiKeys(ref) {
  const executable = path.join(
    repositoryRoot,
    "desktop",
    "node_modules",
    "supabase",
    "dist",
    "supabase.js",
  );
  const result = spawnSync(process.execPath, [executable, "projects", "api-keys", "--project-ref", ref], {
    cwd: repositoryRoot,
    encoding: "utf8",
    windowsHide: true,
  });
  if (result.status !== 0) fail("Supabase API keys could not be loaded through the authenticated CLI.");
  const parsed = JSON.parse(result.stdout);
  if (!Array.isArray(parsed.keys)) fail("Supabase returned an invalid API-key response.");
  return parsed.keys;
}

function assertLinkedProject(expectedRef) {
  const linkedRefPath = path.join(repositoryRoot, "supabase", ".temp", "project-ref");
  let linkedRef;
  try {
    linkedRef = readFileSync(linkedRefPath, "utf8").trim();
  } catch {
    fail("The repository is not linked to a Supabase project.");
  }
  if (linkedRef !== expectedRef) {
    fail("--project-ref does not match the Supabase project linked to this repository.");
  }
}

function selectApiKey(keys, predicate) {
  const match = keys.find(predicate);
  return typeof match?.api_key === "string" && match.api_key ? match : null;
}

async function enrollAuthenticator(url, publishable, accessToken) {
  const enrollment = await requestJson("Canary authenticator enrollment", `${url}/auth/v1/factors`, {
    method: "POST",
    headers: apiHeaders(publishable, accessToken),
    body: JSON.stringify({
      factor_type: "totp",
      friendly_name: "Luna managed canary",
    }),
  });
  if (typeof enrollment.id !== "string" || typeof enrollment.totp?.secret !== "string") {
    fail("Supabase returned an invalid authenticator enrollment.");
  }
  const challenge = await requestJson(
    "Canary authenticator challenge",
    `${url}/auth/v1/factors/${enrollment.id}/challenge`,
    {
      method: "POST",
      headers: apiHeaders(publishable, accessToken),
      body: "{}",
    },
  );
  if (typeof challenge.id !== "string") fail("Supabase returned an invalid authenticator challenge.");
  const verification = await requestJson(
    "Canary authenticator verification",
    `${url}/auth/v1/factors/${enrollment.id}/verify`,
    {
      method: "POST",
      headers: apiHeaders(publishable, accessToken),
      body: JSON.stringify({
        challenge_id: challenge.id,
        code: currentTotp(enrollment.totp.secret),
      }),
    },
  );
  const verifiedAccessToken = verification.access_token ?? accessToken;
  if (typeof verifiedAccessToken !== "string" || !verifiedAccessToken) {
    fail("The verified canary session did not return an access token.");
  }
  return { secret: enrollment.totp.secret, accessToken: verifiedAccessToken };
}

function apiHeaders(apiKey, accessToken = null) {
  const headers = {
    apikey: apiKey,
    "content-type": "application/json",
  };
  if (accessToken) headers.authorization = `Bearer ${accessToken}`;
  return headers;
}

async function requestJson(operation, url, options = {}) {
  let response;
  try {
    response = await fetch(url, options);
  } catch {
    fail(`${operation} could not reach Supabase.`);
  }
  const body = await response.text();
  let parsed = null;
  if (body) {
    try {
      parsed = JSON.parse(body);
    } catch {
      if (response.ok) fail(`${operation} returned an invalid response.`);
    }
  }
  if (!response.ok) {
    const detail = parsed?.msg ?? parsed?.message ?? parsed?.error_description ?? parsed?.error;
    fail(`${operation} failed with HTTP ${response.status}${detail ? `: ${detail}` : "."}`);
  }
  return parsed;
}

function currentTotp(base32Secret) {
  const key = decodeBase32(base32Secret);
  const counter = Math.floor(Date.now() / 30_000);
  const counterBytes = Buffer.alloc(8);
  counterBytes.writeBigUInt64BE(BigInt(counter));
  const digest = createHmac("sha1", key).update(counterBytes).digest();
  const offset = digest[digest.length - 1] & 0x0f;
  const binary = (
    ((digest[offset] & 0x7f) << 24)
    | (digest[offset + 1] << 16)
    | (digest[offset + 2] << 8)
    | digest[offset + 3]
  );
  return String(binary % 1_000_000).padStart(6, "0");
}

function decodeBase32(value) {
  const alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
  let bits = "";
  for (const character of value.toUpperCase().replace(/=+$/u, "")) {
    const index = alphabet.indexOf(character);
    if (index < 0) fail("Supabase returned an invalid authenticator secret.");
    bits += index.toString(2).padStart(5, "0");
  }
  const bytes = [];
  for (let index = 0; index + 8 <= bits.length; index += 8) {
    bytes.push(Number.parseInt(bits.slice(index, index + 8), 2));
  }
  return Buffer.from(bytes);
}

function bootstrapCanaryThroughLinkedDatabase(configuration) {
  const emailLiteral = sqlLiteral(configuration.email);
  const passwordLiteral = sqlLiteral(configuration.password);
  const householdNameLiteral = sqlLiteral(configuration.householdName);
  const validUntilLiteral = sqlLiteral(configuration.validUntil);
  const sql = `
begin;
select set_config('request.jwt.claim.role', 'service_role', true);
do $luna_canary$
declare
  canary_user_id uuid;
  canary_account_id uuid;
  canary_household_id uuid;
begin
  select id into canary_user_id
  from auth.users
  where lower(email) = lower(${emailLiteral}) and is_sso_user = false
  limit 1;

  if canary_user_id is null then
    canary_user_id := gen_random_uuid();
    insert into auth.users (
      instance_id, id, aud, role, email, encrypted_password,
      email_confirmed_at, raw_app_meta_data, raw_user_meta_data,
      created_at, updated_at, confirmation_token, recovery_token,
      email_change_token_new, email_change, email_change_token_current,
      reauthentication_token, is_sso_user, is_anonymous
    ) values (
      '00000000-0000-0000-0000-000000000000', canary_user_id,
      'authenticated', 'authenticated', ${emailLiteral},
      crypt(${passwordLiteral}, gen_salt('bf')), now(),
      jsonb_build_object('provider', 'email', 'providers', jsonb_build_array('email')),
      jsonb_build_object('organiser_name', 'Luna Test Organiser'),
      now(), now(), '', '', '', '', '', '', false, false
    );
  else
    update auth.users
    set encrypted_password = crypt(${passwordLiteral}, gen_salt('bf')),
        email_confirmed_at = coalesce(email_confirmed_at, now()),
        raw_user_meta_data = jsonb_build_object('organiser_name', 'Luna Test Organiser'),
        updated_at = now()
    where id = canary_user_id;
    delete from auth.mfa_factors where user_id = canary_user_id;
    delete from auth.sessions where user_id = canary_user_id;
  end if;

  insert into auth.identities (
    provider_id, user_id, identity_data, provider, created_at, updated_at
  ) values (
    canary_user_id::text, canary_user_id,
    jsonb_build_object(
      'sub', canary_user_id::text,
      'email', ${emailLiteral},
      'email_verified', true,
      'phone_verified', false
    ),
    'email', now(), now()
  )
  on conflict (provider_id, provider) do update
  set identity_data = excluded.identity_data,
      updated_at = now();

  select account_id into canary_account_id
  from public.external_identities
  where provider = 'supabase' and provider_subject = canary_user_id::text;
  if canary_account_id is null then
    raise exception 'The Luna account trigger did not create the canary identity';
  end if;

  select membership.household_id into canary_household_id
  from public.household_memberships as membership
  where membership.account_id = canary_account_id;
  if canary_household_id is null then
    insert into public.households (name, created_by_account_id)
    values (${householdNameLiteral}, canary_account_id)
    returning id into canary_household_id;
    insert into public.household_memberships (household_id, account_id, role)
    values (canary_household_id, canary_account_id, 'household_organiser');
  end if;

  perform public.grant_complimentary_managed_intelligence(
    canary_household_id,
    ${configuration.budgetUsd.toFixed(2)}::numeric,
    ${validUntilLiteral}::timestamptz
  );
end
$luna_canary$;
select
  household.id as household_id,
  household.name as household_name
from auth.users as auth_user
join public.external_identities as identity
  on identity.provider = 'supabase' and identity.provider_subject = auth_user.id::text
join public.household_memberships as membership on membership.account_id = identity.account_id
join public.households as household on household.id = membership.household_id
where lower(auth_user.email) = lower(${emailLiteral});
commit;
`;
  const result = runLinkedSql(sql, [
    configuration.password,
    configuration.email,
  ]);
  const household = singleRow(result.rows);
  if (
    typeof household?.household_id !== "string"
    || typeof household?.household_name !== "string"
  ) {
    fail("The linked database did not return the canary Household.");
  }
  return household;
}

function runLinkedSql(sql, sensitiveValues) {
  const executable = path.join(
    repositoryRoot,
    "desktop",
    "node_modules",
    "supabase",
    "dist",
    "supabase.js",
  );
  const systemTemporaryDirectory = path.resolve(os.tmpdir());
  const queryDirectory = mkdtempSync(path.join(systemTemporaryDirectory, "luna-canary-"));
  const resolvedQueryDirectory = path.resolve(queryDirectory);
  if (!resolvedQueryDirectory.startsWith(`${systemTemporaryDirectory}${path.sep}`)) {
    fail("The temporary canary query directory escaped Windows Temp.");
  }
  const queryFile = path.join(resolvedQueryDirectory, "bootstrap.sql");
  writeFileSync(queryFile, sql, { encoding: "utf8", flag: "wx", mode: 0o600 });
  let result;
  try {
    result = spawnSync(
      process.execPath,
      [executable, "db", "query", "--linked", "--file", queryFile],
      {
        cwd: repositoryRoot,
        encoding: "utf8",
        windowsHide: true,
      },
    );
  } finally {
    unlinkSync(queryFile);
    rmdirSync(resolvedQueryDirectory);
  }
  if (result.status !== 0) {
    const diagnostic = sanitizeDiagnostic(
      `${result.stderr ?? ""}\n${result.stdout ?? ""}`,
      sensitiveValues,
    );
    fail(`The linked database rejected the canary bootstrap transaction${diagnostic ? `: ${diagnostic}` : "."}`);
  }
  let parsed;
  try {
    parsed = JSON.parse(result.stdout);
  } catch {
    fail("The linked database returned an invalid bootstrap response.");
  }
  if (!Array.isArray(parsed.rows)) fail("The linked database bootstrap returned no rows.");
  return parsed;
}

function sqlLiteral(value) {
  return `'${String(value).replaceAll("'", "''")}'`;
}

function sanitizeDiagnostic(value, sensitiveValues) {
  let sanitized = String(value ?? "");
  for (const sensitive of sensitiveValues) {
    if (sensitive) sanitized = sanitized.replaceAll(sensitive, "[redacted]");
  }
  const errorLine = sanitized
    .split(/\r?\n/u)
    .map((line) => line.trim())
    .find((line) => /(?:error|failed|exception)/iu.test(line));
  return errorLine?.slice(0, 500) ?? "";
}

function singleRow(value) {
  if (Array.isArray(value)) return value[0] ?? null;
  return value && typeof value === "object" ? value : null;
}

function saveProtectedCredential(destination, credential) {
  const powershell = [
    "$ErrorActionPreference = 'Stop'",
    "$destination = $env:LUNA_CANARY_CREDENTIAL_PATH",
    "$directory = Split-Path -Parent $destination",
    "New-Item -ItemType Directory -Force -Path $directory | Out-Null",
    "$plain = [Console]::In.ReadToEnd()",
    "$protected = ConvertTo-SecureString -String $plain -AsPlainText -Force",
    "$protected | Export-Clixml -LiteralPath $destination",
  ].join("; ");
  const result = spawnSync("powershell.exe", ["-NoProfile", "-NonInteractive", "-Command", powershell], {
    input: JSON.stringify(credential),
    encoding: "utf8",
    env: { ...process.env, LUNA_CANARY_CREDENTIAL_PATH: destination },
    windowsHide: true,
  });
  if (result.status !== 0) fail("The canary credentials could not be stored with Windows protection.");
}

function fail(message) {
  process.stderr.write(`${message}\n`);
  process.exit(1);
}
