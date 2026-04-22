<!-- Post 3 of 4 — Formato técnico, audiencia técnica. Incluye link al blog post -->

How I use CogniCode for code review workflows

When reviewing PRs, I now ask the AI to do the boring analysis work first. Here's a real workflow:

---

**Step 1: Check architecture health**

```json
{
  "tool": "check_architecture",
  "arguments": {}
}
```

Catches circular dependencies before they become tech debt. Uses Tarjan SCC algorithm.

---

**Step 2: Find hot paths**

```json
{
  "tool": "get_hot_paths",
  "arguments": {
    "min_fan_in": 3
  }
}
```

Functions with high fan-in are critical — they deserve extra scrutiny. This tells you where to focus review attention.

---

**Step 3: Analyze impact of proposed changes**

```json
{
  "tool": "analyze_impact",
  "arguments": {
    "symbol_name": "process_payment",
    "file": "src/billing/stripe.rs",
    "line": 89
  }
}
```

Response tells you: risk level, impacted files, and what functions call this. No more guessing.

---

**Step 4: Trace execution paths**

```json
{
  "tool": "trace_path",
  "arguments": {
    "source": "handle_webhook",
    "target": "update_ledger",
    "max_depth": 8
  }
}
```

Understand the full chain from webhook to database without reading all the intermediate code.

---

**Why this matters for code review:**

Traditional AI: "This looks fine to me"
CogniCode-powered AI: "This function is called by 12 other modules, has a circular dependency risk, and changing it would require updating the billing reconciliation code"

The first response sounds confident. The second is actually useful.

I've been dogfooding this on my own PRs. The quality of AI-assisted feedback is noticeably better when the agent has graph-based code intelligence.

Full breakdown in the blog post: [Link to blog-post-en.md]

What code review workflows have you automated? Curious what others are doing.

#CodeReview #AI #DeveloperTools #TechDebt
