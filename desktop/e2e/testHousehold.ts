import { e2eAccountFixture } from "../src/account/e2eAccountFixture";

export const testHousehold = {
  organiserName: "Sam Rivera",
  email: "sam@example.com",
  password: "correct-horse-battery-staple-7",
  name: "Rivera Household",
  verificationCode: e2eAccountFixture.verificationCode,
  recoveryCode: e2eAccountFixture.recoveryCode,
  replacementPassword: "replacement-horse-battery-staple-8",
} as const;
