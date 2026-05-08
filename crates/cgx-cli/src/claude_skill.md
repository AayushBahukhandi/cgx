---
name: cgx
description: "Index any Git repo as a knowledge graph and query it — architecture, blast radius, dependencies, hotspots, ownership."
trigger: /cgx
---

# /cgx

Turn any Git repo into a queryable knowledge graph. Answers architecture questions in milliseconds with zero file-reading.

## Usage

```
/cgx                          # index + query current directory
/cgx <path>                   # index + query a specific path
/cgx github:owner/repo        # clone from GitHub, index, query
/cgx query "<question>"       # query mode on already-indexed repo
/cgx hotspots                 # highest-risk files in current repo
/cgx blast-radius <symbol>    # what breaks if this changes?
```

## What to do when invoked

### Step 1 — Ensure cgx is installed

```bash
cgx --version 2>/dev/null || echo "NOT_INSTALLED"
```

If not installed:
```bash
brew install aayushbahukhandi/cgx/cgx 2>/dev/null || cargo install cgx-cli
```

The cgx binary on this machine: `{{CGX_PATH}}`

### Step 2 — Analyze the repo (if needed)

```bash
# Current directory:
cgx analyze --verbose

# Specific path:
cgx analyze /path/to/repo --verbose

# GitHub repo (clones into ~/.cgx/clones/):
cgx analyze github:owner/repo --verbose
```

Present a clean summary — don't dump raw output.

### Step 3 — Answer questions with cgx queries (NEVER open files)

```bash
cgx summary                           # orientation — always run first
cgx query find "<name>"               # locate any symbol
cgx query find "<name>" --kind=Function
cgx query blast-radius "<function>"   # change impact (run BEFORE any edit)
cgx query deps "<node>"               # what does this depend on
cgx query chain "<A> -> <B>"          # trace call path
cgx hotspots                          # highest churn × coupling files
cgx query owners <path>               # git blame ownership
cgx query search "<phrase>"           # full-text search
cgx query community <id>              # explore a cluster
cgx query dead-code                   # find unused exports
```

**Token budget:** 1 cgx query ≈ 200–800 tokens. Reading 1 source file ≈ 2,000–15,000 tokens.

### Natural language → cgx command translation

| User says | Run |
|---|---|
| "what calls X" / "who uses X" | `cgx query find X` → `cgx query blast-radius X` |
| "what breaks if I change X" | `cgx query blast-radius X` |
| "how does A connect to B" | `cgx query chain "A -> B"` |
| "riskiest files" / "what to watch out for" | `cgx hotspots` |
| "who owns X" / "who wrote X" | `cgx query owners X` |
| "find all usages of X" | `cgx query find X` |
| "is X dead code" | `cgx query dead-code` filtered for X |
| "show me the architecture" | `cgx summary` then `cgx query community` on top clusters |

## Output format

- Summarize results in plain English — never dump raw JSON
- Lead with the key fact: "X has 14 callers; changing it is HIGH risk"
- Offer to drill deeper: "Want me to trace the call chain from X to Y?"

## Permanent editor setup

```bash
cgx setup    # writes MCP config for Claude Code, Cursor, VS Code, Windsurf, Zed
```

After editor restart, cgx tools are available without running this skill.
