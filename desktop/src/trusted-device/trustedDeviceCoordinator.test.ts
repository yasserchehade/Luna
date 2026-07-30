import assert from "node:assert/strict";
import test from "node:test";
import type { TrustedDeviceRecord } from "../account/accountService";
import { trustedDeviceEnrollmentMode } from "./trustedDeviceCoordinator";

test("verified MFA with no prior device resumes first-device enrollment", () => {
  assert.equal(trustedDeviceEnrollmentMode("verified", []), "firstVerified");
});

test("an existing device keeps a new machine on the recovery path", () => {
  const existing: TrustedDeviceRecord = {
    id: "device-1",
    label: "Existing device",
    publicKey: "age1existing",
    keyEpoch: 1,
    status: "active",
  };
  assert.equal(trustedDeviceEnrollmentMode("verified", [existing]), "recovery");
});

test("an account without MFA still uses complete first-device setup", () => {
  assert.equal(trustedDeviceEnrollmentMode("unenrolled", []), "first");
});
