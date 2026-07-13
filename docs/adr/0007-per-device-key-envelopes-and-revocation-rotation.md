---
status: accepted
---

# Wrap Household keys per device and rotate them on revocation

Each Trusted Device creates its own X25519 key pair and keeps its private key plus the current Household memory key in the operating-system credential vault. A local Device PIN, hashed with Argon2, unlocks that material for the current Luna session; it is not the Luna Account password.

Luna stores only public keys and encrypted age envelopes: one envelope per active device and a separate envelope protected by the offline Recovery Key. Replacement recovery is two-phase: local material remains pending until the service accepts the device at the current key epoch. Account-password recovery therefore cannot decrypt Household memory or accidentally complete Trusted Device recovery.

Device Revocation is an organiser-authorised, MFA-protected rotation. The local core prepares a new Household key and an envelope for every retained active device, the service atomically revokes the target and advances the key epoch, and only then does the initiating device activate its pending key. This prevents revoked devices from reading future state without pretending Luna can erase information they already opened, at the cost of requiring the Recovery Key when the recovery envelope must be rotated.
