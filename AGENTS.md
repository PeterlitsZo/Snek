# AGENTS.md

## Behavior

- Use the user's language for conversations and plans.
- Use English for code and other implementation artifacts.
- Use only open-source third-party dependencies; do not introduce private libraries or repositories.
- Do not write unit tests and document until user ask you to do so.

## Plans

- Write plan in `plans/<date-like-20261113>/<seq-like-01>/main.md`.

## Git Commit Message

Git commit messages use the following format:

```
<type>: <subject>
```

Here, `<type>` describes the nature of the change, such as `feat`, `fix`,
`docs`, `chore`, or `refactor`.

The `<subject>` should be a short description of the change in English. Keep it
concise and clear. You need to review all changed files and summarize the most
important change (for example, if the project includes both a refactor and a
documentation update, the subject should describe the refactor rather than the
documentation update). The subject MUST end with an English full stop, and its
first letter MUST be capitalized.
