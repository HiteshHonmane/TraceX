# Git Workflow

Git history is an engineering record. Keep it safe, focused, and useful for
review, rollback, and future investigation.

## Branches

- Create short-lived branches from the current integration branch.
- Use descriptive names such as `feature/session-recovery` or
  `fix/rotation-boundary`.
- Keep a branch focused on one product outcome.
- Rebase or merge according to repository policy; never rewrite shared history
  without coordination.

## Commits

Each commit should represent one logical change and leave the repository in a
coherent state when practical. Commit messages explain why the change exists.

Suggested format:

```text
area: imperative summary

Why the change is needed and any non-obvious tradeoff.
```

Do not mix generated artifacts, broad formatting, dependency upgrades, and
feature logic unless they are inseparable.

## Before Commit

- Inspect `git diff` and `git status`.
- Remove secrets, local paths, temporary files, and debug output.
- Format code and run the tests appropriate to the change.
- Update documentation and `DECISIONS.md` when required.
- Preserve unrelated user changes in a dirty worktree.

## Pull Requests

Pull requests should be small enough to review as one idea. Include the problem,
solution, tradeoffs, validation, screenshots for UI changes, and performance
evidence for hot-path work. Link issues and ADRs where they provide context.

## Dependencies and Generated Files

Dependency changes must include the manifest and lockfile together and explain
the new capability. Review transitive packages and licenses. Generated files are
committed only when the repository explicitly treats them as source artifacts.

## Safety

- Never use destructive reset or checkout commands on work you do not own.
- Never force-push a shared branch without explicit coordination.
- Never commit credentials, private logs, customer data, or machine-specific
  configuration.
- Prefer a revert to rewriting public history after a bad merge.

## Release Tags

Release tags must refer to reviewed, reproducible commits. Version, changelog,
artifacts, and tag must agree. Signing and release automation will be documented
before the first public production release.
