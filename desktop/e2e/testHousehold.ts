import { e2eAccountFixture } from "../src/account/e2eAccountFixture";

export const testHousehold = {
  organiserName: "Sam Rivera",
  email: "sam@example.com",
  password: "correct-horse-battery-staple-7",
  name: "Rivera Household",
  id: e2eAccountFixture.householdId,
  verificationCode: e2eAccountFixture.verificationCode,
  recoveryCode: e2eAccountFixture.recoveryCode,
  replacementPassword: "replacement-horse-battery-staple-8",
  authenticatorCode: e2eAccountFixture.authenticatorCode,
  authenticatorSecret: e2eAccountFixture.authenticatorSecret,
  devicePin: "246810",
  replacementDevicePin: "135790",
} as const;
