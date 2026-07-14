---
status: accepted
---

# Let an unlocked Trusted Device replace a lost Recovery Key

Recovery Key Replacement requires a Household Organiser, authenticator verification within the previous five minutes and an unlocked Trusted Device. Each newly enrolled Trusted Device therefore holds a separate signing key in the operating-system credential vault, and Luna's service stores only its public verification key. The device signs the proposed recovery envelope, recovery verification key, Household identity, current key epoch and current recovery verification key before the service atomically replaces recovery authority. Binding the signed request to the current recovery verifier prevents a previously signed replacement from being replayed.

Replacement and Recovery Key device enrolment both lock the Household recovery-authority row while validating and writing. A device enrolment using the previous Recovery Key therefore cannot commit after a concurrent replacement has made that authority stale.

The lost Recovery Key is never retrieved, and Luna Account access alone cannot authorise replacement. A successful replacement makes the previous Recovery Key unusable for future device enrolment. If every Trusted Device and the Recovery Key are lost, Luna still cannot recover the Household key.

Trusted Devices enrolled before device signing keys existed cannot be upgraded securely using Luna Account authentication alone, because the service has no prior device-held signing authority against which to verify an upgrade. During beta those devices must be re-enrolled; Luna must not silently accept a weaker account-only migration.
