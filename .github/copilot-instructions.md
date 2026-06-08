# Copilot Safety Guardrails (Repository)

These rules are mandatory for all assistant-generated commands in this repository.

## Blocked command patterns
- Do not generate or execute `zsh -c` + `eval` command chains.
- Do not generate or execute download-and-run flows in one step (for example: `curl|wget` -> `chmod +x` -> execute).
- Do not execute newly downloaded binaries from temporary directories such as `/tmp`.
- Do not `source` shell snapshot scripts from hidden folders unless the user explicitly asks.
- Do not run browser-quarantine simulation commands unless the user explicitly asks.

## Required approval gate
Before any command that includes one or more of the following, ask for explicit approval and wait:
- remote execution over `ssh`
- downloading executable artifacts
- changing executable permissions (`chmod +x`) on downloaded files
- running unsigned or newly downloaded binaries

Approval phrase required from user: `批准执行`.
Without this phrase, provide a read-only verification plan only.

## Safe execution style
- Prefer step-by-step, auditable commands over long one-liners.
- Separate phases: download, verify, execute.
- Prefer fixed project directories over temporary execution paths.
- Default to non-destructive and read-only validation first (hash, signature, metadata).
