# Issue tracker: GitHub

Specifications and tickets for this repository live in GitHub Issues. Use the GitHub CLI from this repository for issue operations.

- Publishing to the tracker means creating a GitHub issue.
- Fetch tickets with `gh issue view <number> --comments`.
- Infer the repository from its Git remote.
- Pull requests are not treated as incoming feature requests.
- Prefer native GitHub blocking relationships. When unavailable, use a `Blocked by: #<number>` section in the ticket body.
- A ticket is ready to implement only when all its blockers are closed.
