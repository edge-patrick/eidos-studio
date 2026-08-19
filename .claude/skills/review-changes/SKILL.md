---
name: review-changes
description: Use when user asks for a review of recent code changes or a feature implementation. Search for real and important problems by defaut. Advice and suggestions of minor or nice to have improvements can be provided, but not put above what actually matters.
---

# Review Changes

Review the requested code. Look for problems that could affect the app or weaken its foundation. Do not change the code unless the user asks for fixes.

## Review process

1. Read the user's request and the repository instructions.
2. Review the files, commit, branch, or feature named by the user. If they ask about current changes, inspect staged, unstaged, and relevant untracked files.
3. Read enough nearby code and tests to understand how the change is used.
4. Run focused tests or checks when they help confirm a problem.

Look for:

- Drifting from established architecture and conventions.
- Broken behavior, crashes, or regressions.
- Data loss, bad state, or unsafe migrations.
- Security or privacy problems.
- Error, async, or lifecycle bugs.
- Mismatches between the UI, backend, storage, APIs, and types.
- Changes that break the project's architecture or create a risky base for likely next steps.

Only report a finding when you can explain how it could cause a real problem. This is a new app, so protect its foundation. It also has one user, so do not ask for enterprise scale, multi-user systems, or extra abstractions without a current need.

Do not report style preferences, naming, harmless duplication, or missing tests by themselves. Put useful but nonessential ideas under **Recommendations**.

## Report the result

List findings from most to least serious. Use these labels:

- **Critical**: Likely data loss, serious security exposure, or an app-wide failure with no reasonable recovery.
- **High**: A core feature can fail, give wrong results, corrupt state, or expose sensitive data.
- **Medium**: A real problem affects a smaller case or has a workaround.

For each finding, give the file and line, explain what is wrong and when it happens, and propose a clear fix. Use plain and simple English. Do not create Low findings; use Recommendations instead.

If there are no important problems, say so. Do not invent findings.
