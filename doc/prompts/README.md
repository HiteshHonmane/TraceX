# TraceX Task Prompts

These prompts give AI contributors a consistent operating method. They supplement
`AGENTS.md`; they never override product, architecture, or engineering rules.

## Choose One Primary Prompt

- `feature.prompt.md` - new user-visible behavior
- `bugfix.prompt.md` - reproduce and correct a defect
- `refactor.prompt.md` - improve structure without changing behavior
- `review.prompt.md` - inspect a proposed or existing change
- `performance.prompt.md` - investigate or improve measurable performance
- `architecture.prompt.md` - make a cross-cutting or hard-to-reverse decision

For mixed work, choose the prompt matching the primary outcome and incorporate
the relevant checks from the others. Do not use a prompt label to broaden scope.

## Common Completion Contract

Every task output must state:

1. outcome and affected behavior;
2. assumptions and decisions;
3. files or subsystems changed;
4. verification performed and exact results;
5. remaining risks or follow-up work.

Never claim a check passed when it was not run.
