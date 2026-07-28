---
status: accepted
---

# Stage Originals in a visible Cabinet folder

Luna will preserve each accepted Original in the Cabinet's visible `Incoming` folder until a Filing Decision moves it to its confirmed Cabinet Destination. Each Original retains its filename in a checksum-named child folder (`Incoming/<SHA-256>/<original filename>`), so files with the same name cannot overwrite one another. We chose a Cabinet folder over app-data staging so an Original remains household-owned, human-readable and usable outside Luna even before it is filed.

If the configured Cabinet becomes unavailable after an Original is staged, Luna keeps the confirmed Cabinet Destination and the staged Original unchanged. Document Handling exposes one recoverable waiting state and checksum-bound Review Card Evidence. Luna resumes only against that configured Cabinet and releases staging only after destination verification and durable Audit Event recording. The encrypted material in this flow is Luna's protected handling metadata; the Original remains deliberately visible under this ADR.
