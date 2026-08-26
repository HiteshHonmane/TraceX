# AI Review Checklist

> This document is the final quality gate for every contribution to TraceX.
>
> Every AI agent MUST perform this review before considering a task complete.
>
> If any applicable item fails, the task is NOT complete.

---

# Rule Zero

Working code is NOT successful code.

A feature is complete only when it:

- Solves the intended problem
- Respects the architecture
- Maintains performance
- Is easy to understand
- Is easy to extend
- Does not introduce technical debt

---

# 1. Requirement Validation

Before writing code, verify:

- [ ] I have read `doc/vision.md`
- [ ] I have read `doc/01-Product-Requirements.md`
- [ ] I have read `doc/02-System-Architecture.md`
- [ ] I understand the feature requirements.
- [ ] I am not making assumptions about product behavior.
- [ ] My implementation aligns with TraceX's mission.

If any answer is NO,

STOP.

---

# 2. Architecture Review

- [ ] Does this follow the defined architecture?
- [ ] Does dependency direction remain correct?
- [ ] Does this avoid introducing tight coupling?
- [ ] Can this feature evolve independently?
- [ ] Does it fit naturally into the existing modules?

If not,

Redesign before implementation.

---

# 3. Module Review

Ask:

Does this belong here?

- [ ] Correct crate?
- [ ] Correct module?
- [ ] Correct responsibility?

If a module starts doing unrelated work,

split it.

---

# 4. Single Responsibility Review

Every new type should have one reason to change.

- [ ] One purpose
- [ ] One responsibility
- [ ] Clear ownership

If multiple responsibilities exist,

refactor.

---

# 5. UI Review

Confirm:

- [ ] UI only displays data.
- [ ] UI only collects user input.
- [ ] UI dispatches actions.

UI must NOT:

- [ ] Parse logs
- [ ] Search logs
- [ ] Read serial ports
- [ ] Save files
- [ ] Manage sessions
- [ ] Execute business logic

---

# 6. Performance Review

Ask:

Can this become slow?

Review:

- [ ] Large allocations
- [ ] String copies
- [ ] Vector cloning
- [ ] Blocking operations
- [ ] Excessive locking
- [ ] Repeated parsing

If unnecessary work exists,

remove it.

---

# 7. Memory Review

Verify:

- [ ] Borrow instead of clone where possible.
- [ ] Avoid unnecessary heap allocations.
- [ ] Avoid duplicate copies of large data.
- [ ] Large logs are streamed instead of duplicated.

---

# 8. Threading Review

Verify:

- [ ] UI thread never blocks.
- [ ] Disk IO is asynchronous.
- [ ] Network IO is asynchronous.
- [ ] Serial IO is asynchronous.
- [ ] Heavy computation is offloaded.
- [ ] Long-running tasks do not freeze the application.

---

# 9. Event System Review

Ask:

Should this communicate through events?

- [ ] No unnecessary direct dependencies.
- [ ] Event Bus used where appropriate.
- [ ] Modules remain independent.

---

# 10. Error Handling Review

Verify:

- [ ] No ignored errors.
- [ ] No silent failures.
- [ ] Errors contain meaningful context.
- [ ] User-facing errors are understandable.

Forbidden:

- unwrap()
- expect()
- panic!()

Except:

- Tests
- Explicit prototypes

---

# 11. Code Quality Review

Review:

- [ ] Functions are easy to read.
- [ ] Variables have meaningful names.
- [ ] Public APIs are documented.
- [ ] Dead code removed.
- [ ] TODOs are intentional.
- [ ] No commented-out code.

---

# 12. Duplication Review

Ask:

Does this logic already exist?

- [ ] No duplicated algorithms.
- [ ] No duplicated utilities.
- [ ] No duplicated parsing logic.
- [ ] No duplicated UI logic.

If duplication exists,

extract shared functionality.

---

# 13. Dependency Review

Before adding a dependency:

- [ ] Standard library cannot solve it.
- [ ] Existing dependencies cannot solve it.
- [ ] Dependency is actively maintained.
- [ ] Dependency is lightweight.
- [ ] Dependency adds clear value.

Never add dependencies for convenience alone.

---

# 14. API Review

Every public API should answer:

- What does it do?
- Why does it exist?
- Is it reusable?
- Is it minimal?
- Will it remain stable?

Avoid exposing implementation details.

---

# 15. Testing Review

Verify:

- [ ] Unit tests added.
- [ ] Existing tests pass.
- [ ] Edge cases covered.
- [ ] Regression tests added for bug fixes.

Untested critical logic is incomplete.

---

# 16. Documentation Review

Confirm:

- [ ] Public APIs documented.
- [ ] Architecture updated if required.
- [ ] Feature documentation updated.
- [ ] Examples updated if behavior changed.

Documentation is part of the implementation.

---

# 17. Cross Platform Review

Confirm compatibility with:

- [ ] Windows
- [ ] Linux
- [ ] macOS

Avoid platform-specific assumptions.

---

# 18. Security Review

Verify:

- [ ] Validate external input.
- [ ] No hardcoded secrets.
- [ ] Safe file handling.
- [ ] Safe path handling.
- [ ] Safe serialization.

---

# 19. Logging Review

Confirm:

- [ ] Meaningful logs added where appropriate.
- [ ] No debug println! statements.
- [ ] Structured logging used.
- [ ] Sensitive data is not logged.

---

# 20. Maintainability Review

Ask yourself:

Will another engineer understand this six months from now?

- [ ] Code is readable.
- [ ] Modules are cohesive.
- [ ] Responsibilities are clear.
- [ ] Architecture remains clean.

---

# Final Decision Matrix

Before marking the task complete, answer:

Can this implementation be:

- Easier?
- Smaller?
- Faster?
- More reusable?
- More maintainable?

If YES,

improve it first.

---

# Definition of Done

A task is complete only if:

- [ ] Requirements satisfied
- [ ] Architecture respected
- [ ] Performance maintained
- [ ] Tests passing
- [ ] Documentation updated
- [ ] Code reviewed against this checklist
- [ ] No unnecessary technical debt introduced

---

# Engineering Philosophy

Good engineers write working code.

Great engineers write code that remains easy to change.

Exceptional engineers make future changes easier than present ones.

Every contribution to TraceX should leave the codebase in a better state than it was found.

> **Leave the campsite cleaner than you found it.**
