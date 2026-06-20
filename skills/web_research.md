---
name: web-research
description: Use when a task requires looking up documentation, finding a library, checking an API spec, or researching a solution before implementing it. Covers effective search query construction, evaluating sources, fetching and extracting relevant content, and synthesizing findings into a decision or implementation plan. Load before any web_search or fetch_url call on a research task.
---

# Web Research

## Overview

Research is the step between "I don't know how to do this" and "I'm ready to implement." The core principle: **research with a question, synthesize before coding, and stop when you have enough to proceed.** Endless research is procrastination; no research is recklessness. Find the sweet spot.

## Query Construction

### Specific Over Generic
```
# BAD — too generic, will get noise
"python web framework"

# GOOD — specific, targets the exact need
"FastAPI dependency injection middleware example 2024"
```

### Include Context in Queries
- **Language/framework**: "Rust" not just "how to parse JSON"
- **Version**: "React 18" not just "React"
- **Error message**: Include the exact error text in quotes
- **Specific feature**: "WebSocket reconnection" not "real-time"

### Query Patterns for Common Tasks
| Task | Query pattern |
|------|---------------|
| How-to | `"<framework> <task> example"` |
| API docs | `"<library> <function name> documentation"` |
| Error fix | `"<exact error message>"` |
| Comparison | `"<option A> vs <option B> <criteria>"` |
| Best practice | `"<technology> <task> best practices"` |

## Source Quality Hierarchy

Trust sources in this order:

1. **Official documentation** — the canonical source. Always start here.
2. **Source code** — the implementation is the truth when docs are wrong.
3. **Reputable tutorials** — MDN, Real Python, Rust Book, Go by Example.
4. **Stack Overflow** — good for specific problems, check the date and votes.
5. **Blog posts** — variable quality, check the author's credibility.
6. **Reddit/forums** — anecdotal, useful for "has anyone done X?" questions.

**Red flags for unreliable sources:**
- No date or older than 2 years for rapidly evolving tech
- No code examples, just prose
- Contradicts official docs without explanation
- "Works on my machine" without explaining why

## Fetching Documentation Pages

### Navigation Strategy
1. Start with the docs homepage or API reference index
2. Use the search function within the docs site (often better than web search)
3. Navigate to the specific section you need
4. If the page is long, search within it for the function/concept name

### Ignoring Nav Chrome
Documentation sites have navigation, sidebars, footers, and ads. When fetching a page:
- The tool strips HTML automatically, but the text may still have nav remnants
- Focus on the section relevant to your question
- Skip introductory paragraphs — go straight to the API reference or code examples

## Extracting Signal

When reading a fetched page, focus on:

1. **Code examples** — the most reliable information. Copy the pattern, adapt it.
2. **Parameter descriptions** — what's required, what's optional, default values
3. **Return types and error cases** — what you get back and what can go wrong
4. **Version notes** — "deprecated in v3, use X instead" is critical

**Skip:**
- Marketing language ("powerful", "easy-to-use")
- Long conceptual introductions when you need the API reference
- Unrelated sections of the page

## Evaluating Conflicting Sources

When sources disagree:

1. **Check the date.** The newer source is more likely correct for actively maintained projects.
2. **Check the version.** A tutorial for v2 may not apply to v3.
3. **Check the official source.** The project's own docs or source code wins.
4. **Test it.** If you can't resolve the conflict by reading, write a small test.

## Synthesizing Before Coding

After research, before implementation:

1. **Write a brief summary** of what you found. One paragraph per key finding.
2. **State the decision.** "I will use library X because Y."
3. **Note the tradeoffs.** "X is simpler but doesn't support Z."
4. **List the steps.** "Install X, configure Y, implement Z using this pattern."

This summary becomes your implementation plan. If you can't write it, you don't understand the problem well enough yet.

## Knowing When You Have Enough

**You have enough when:**
- You can write the implementation steps without looking anything else up
- You know which library/function/approach to use
- You understand the main gotchas and edge cases
- You've confirmed the approach works with your version and constraints

**You need more research when:**
- You're still comparing multiple approaches without a clear winner
- You don't know how to handle a specific error case
- The documentation is ambiguous and you haven't tested it

**Stop researching when:**
- You've spent more than 15 minutes without a clear direction
- You're reading the same information in different sources
- You have enough to start — you can research specific details as they come up

## Anti-Patterns

- **Researching without a question.** Browsing docs aimlessly wastes time. Always have a specific question.
- **Accepting the first result.** The top Google result isn't always the best. Check the source quality.
- **Not checking the date.** A 2019 tutorial on a 2024 framework will lead you astray.
- **Copying code without understanding it.** If you can't explain what the code does, don't use it.
- **Endless research.** If you've found a viable approach, start implementing. You can research more as needed.
- **Not testing the approach.** A small spike (see `task_decomposition`) validates research faster than reading more articles.
