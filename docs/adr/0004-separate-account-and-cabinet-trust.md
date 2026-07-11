---
status: accepted
---

# Separate account recovery from cabinet decryption

A Luna account coordinates identity, household membership and trusted-device enrolment, but it cannot decrypt household memory by itself; decryption recovery requires an existing trusted device or the offline recovery key. This deliberately sacrifices provider-assisted recovery when every household key is lost so a compromised account or Luna service cannot expose the cabinet.
