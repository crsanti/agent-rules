---
# Narrows gentle-ai's broad ".env.*" deny so that template files stay usable
# (for scaffolding new projects/features) while real secrets remain blocked.
# Accessible ON PURPOSE (NOT denied below): .env.template, .env.example.
# gentle-ai install/sync REPLACES the permissions.deny array on every run, so
# re-run `agent-rules apply` after each sync to re-apply this.
# This is an explicit denylist: only the variants in `ensure` are blocked; add
# any new secret variant there yourself. Anything omitted stays editable.
target: ~/.claude/settings.json
format: json
json_path: permissions.deny
---
{
  "remove": [
    "Read(.env.*)",
    "Edit(.env.*)"
  ],
  "ensure": [
    "Read(.env)",
    "Edit(.env)",
    "Read(.env.local)",
    "Edit(.env.local)",
    "Read(.env.development)",
    "Edit(.env.development)",
    "Read(.env.production)",
    "Edit(.env.production)"
  ]
}
