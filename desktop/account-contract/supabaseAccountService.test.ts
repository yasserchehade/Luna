import assert from "node:assert/strict";
import path from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";
import { createClient } from "@supabase/supabase-js";
import { SupabaseAccountService } from "../src/account/supabaseAccountService";

const localConfig = readLocalSupabaseConfig();
const supabaseUrl = process.env.SUPABASE_URL ?? localConfig.API_URL;
const publishableKey = process.env.SUPABASE_PUBLISHABLE_KEY ?? localConfig.PUBLISHABLE_KEY;

test("a verified Luna Account returns to the same Household after signing in again", async () => {
  assert.ok(supabaseUrl, "SUPABASE_URL is required");
  assert.ok(publishableKey, "SUPABASE_PUBLISHABLE_KEY is required");

  const accountService = new SupabaseAccountService(supabaseUrl, publishableKey);
  const email = `sam.${crypto.randomUUID()}@example.com`;
  const password = "correct-horse-battery-staple-7";

  await accountService.register({ organiserName: "Sam Rivera", email, password });
  const verificationCode = await readVerificationCode(email);
  await accountService.verifyEmail(email, verificationCode);

  const created = await accountService.createHousehold("Rivera Household");
  assert.equal(created.organiserName, "Sam Rivera");
  assert.equal(created.householdName, "Rivera Household");

  await accountService.signOut();
  const returned = await accountService.signIn(email, password);
  assert.deepEqual(returned, created);

  const organiserClient = createClient(supabaseUrl, publishableKey, { auth: { persistSession: false } });
  assert.equal((await organiserClient.auth.signInWithPassword({ email, password })).error, null);
  const organiserHouseholds = await organiserClient.from("households").select("id");
  assert.equal(organiserHouseholds.error, null);
  assert.deepEqual(organiserHouseholds.data, [{ id: created.householdId }]);

  const outsiderEmail = `alex.${crypto.randomUUID()}@example.com`;
  const outsiderPassword = "another-correct-password-7";
  const outsiderClient = createClient(supabaseUrl, publishableKey, { auth: { persistSession: false } });
  assert.equal((await outsiderClient.auth.signUp({
    email: outsiderEmail,
    password: outsiderPassword,
    options: { data: { organiser_name: "Alex Morgan" } },
  })).error, null);
  const outsiderCode = await readVerificationCode(outsiderEmail);
  assert.equal((await outsiderClient.auth.verifyOtp({
    email: outsiderEmail,
    token: outsiderCode,
    type: "email",
  })).error, null);
  const outsiderHouseholds = await outsiderClient.from("households").select("id");
  assert.equal(outsiderHouseholds.error, null);
  assert.deepEqual(outsiderHouseholds.data, []);

  await accountService.signOut();
  assert.deepEqual(
    await accountService.register({ organiserName: "Someone Else", email, password }),
    { email },
  );
});

async function readVerificationCode(email: string): Promise<string> {
  const response = await fetch(`http://127.0.0.1:54324/api/v1/search?query=to:${encodeURIComponent(email)}`);
  assert.equal(response.ok, true, "local verification email should be available");
  const search = await response.json() as { messages?: Array<{ ID: string }> };
  const messageId = search.messages?.[0]?.ID;
  assert.ok(messageId, `verification email for ${email} was not found`);

  const messageResponse = await fetch(`http://127.0.0.1:54324/api/v1/message/${messageId}`);
  assert.equal(messageResponse.ok, true, "local verification email should be readable");
  const message = await messageResponse.json() as { Text?: string; HTML?: string };
  const code = `${message.Text ?? ""} ${message.HTML ?? ""}`.match(/\b\d{6}\b/)?.[0];
  assert.ok(code, "verification email should contain a six-digit code");
  return code;
}

function readLocalSupabaseConfig(): { API_URL?: string; PUBLISHABLE_KEY?: string } {
  if (process.env.SUPABASE_URL && process.env.SUPABASE_PUBLISHABLE_KEY) return {};

  const command = path.resolve("node_modules", "supabase", "dist", "supabase.js");
  const status = spawnSync(
    process.execPath,
    [command, "status", "--output", "json", "--workdir", path.resolve("..")],
    { encoding: "utf8" },
  );
  assert.equal(status.status, 0, status.stderr || "local Supabase must be running");
  return JSON.parse(status.stdout) as { API_URL?: string; PUBLISHABLE_KEY?: string };
}
