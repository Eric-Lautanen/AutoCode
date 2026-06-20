---
name: ux-principles
description: Use when making decisions about how a feature should behave from the user's perspective - flows, feedback, error states, loading states, empty states, and overall usability. Load when designing a new feature flow, reviewing UX, or when a user complains something is confusing or hard to use.
---

# UX Principles

## Overview

Good UX means the user can accomplish their goal without thinking about the interface. Every interaction should feel predictable, every outcome should be visible, and every mistake should be recoverable. These principles aren't about visual design — they're about behavior: what happens when the user clicks, what they see while waiting, and how the system communicates. Apply these to any interface: web, mobile, CLI, or API.

For visual design decisions, see `ui_design_fundamentals.md`. For component-level API design, see `component_design.md`.

## Feedback: Every Action Needs a Response

Nothing is invisible. Every user action must produce a visible, understandable response.

| User Action | Response |
|-------------|----------|
| Click a button | Visual change (pressed state), then action result |
| Submit a form | Loading indicator → success confirmation or error message |
| Delete an item | Item removed with animation + undo option |
| Save changes | "Saved" toast or indicator |
| Network request | Loading state (spinner, skeleton, progress bar) |
| Invalid input | Inline error near the field, not a generic alert |

**Anti-pattern**: The user clicks "Save" and nothing visible happens. They click again. They click three more times. Now they've saved five times. Always show that something happened.

## Error Messages

Say what went wrong and what the user can do about it.

| Bad | Good |
|-----|------|
| "Error 500" | "We couldn't save your changes. Try again in a moment." |
| "Invalid input" | "Email address must include an @ sign" |
| "Operation failed" | "File must be smaller than 10MB. Your file is 25MB." |
| "Something went wrong" | "Your session expired. Please log in again." |

**Rules for error messages:**
- **Specific**: What exactly went wrong
- **Actionable**: What the user can do to fix it
- **Polite**: Not blaming the user ("Invalid email" → "Please enter a valid email address")
- **Near the problem**: Inline errors next to the relevant field, not a banner at the top

## Empty States

A blank screen tells the user nothing. Every empty state should explain why it's empty and what to do next.

| Context | Empty State |
|---------|------------|
| New account, no data | "You haven't created any projects yet. [Create your first project]" |
| Search with no results | "No results for 'xyzq'. Try different keywords or check your spelling." |
| Filtered list, no matches | "No items match your current filters. [Clear filters]" |
| No notifications | "You're all caught up! No new notifications." |

**Pattern**: Illustration or icon + explanation + action button. Never just a blank page.

## Loading States

### Choose the Right Pattern

| Pattern | When to use |
|---------|-------------|
| **Skeleton screen** | Content with known layout (cards, lists, tables). Shows structure before data arrives. |
| **Spinner** | Short waits (<1s), small areas, or when layout is unknown |
| **Progress bar** | Determinate progress (file upload, multi-step process) |
| **Button loading state** | After form submission — disable button, show spinner inside it |
| **Optimistic update** | When the action is almost certain to succeed (like, star, toggle) |

**Skeleton screens > spinners** for content areas. They reduce perceived wait time and prevent layout shift.

### Rules
- Show loading state within 200ms of the action (use a delay for fast responses to avoid flash)
- Disable the trigger during loading (prevent double-submits)
- If loading takes >5s, show a progress indicator or estimated time
- Never show a loading state that looks like the final content (confusing when it changes)

## Progressive Disclosure

Show what's needed now. Reveal complexity on demand.

- **Default view**: The 80% case. Most users never need more.
- **Advanced options**: Behind an "Advanced" toggle or collapsible section.
- **Tooltips**: Secondary information on hover/focus, not always visible.
- **Wizards**: Break complex setup into steps. Don't show all 20 fields at once.

**Anti-pattern**: Showing every option, setting, and field at once. The user is overwhelmed and can't find what they need.

## Affordance

Interactive things should look interactive. Static things shouldn't.

- **Buttons** look clickable (raised, colored, cursor: pointer)
- **Links** look clickable (underlined, colored, cursor: pointer)
- **Text** looks readable (not clickable unless it is)
- **Cards** that are clickable have hover states; cards that aren't, don't

**Test**: Can you tell what's clickable without moving your mouse? If not, the affordance is wrong.

## Forgiveness

Let users undo. Don't punish mistakes.

- **Undo over confirmation**: Instead of "Are you sure you want to delete?", delete immediately and show an undo option for 5-10 seconds.
- **Confirm destructive actions**: When undo isn't possible (permanent delete, sending email), use a clear confirmation dialog.
- **Auto-save**: Don't make users remember to save. Save automatically, show "Saved" indicator.
- **Form recovery**: Don't lose form data on validation errors. Keep what they typed.
- **Back button safe**: The back button should never cause data loss or errors.

## Consistency

The same action should always look and work the same way across the application.

- **Same icon** for the same action everywhere (don't use a trash can in one place and an X in another for delete)
- **Same button style** for the same level of action (primary actions always look primary)
- **Same keyboard shortcut** for the same action (Ctrl+S always saves)
- **Same terminology** (don't call it "Folder" in one place and "Collection" in another for the same thing)

**Inconsistency** forces the user to re-learn the interface on every page. It's the fastest way to erode trust.

## Common UX Anti-Patterns

| Anti-Pattern | Why It's Bad | Fix |
|---|---|---|
| Mystery meat navigation | User doesn't know what an icon does until they hover | Use text labels, not icons alone |
| Disappearing content | Content that was visible is now gone with no explanation | Show empty state with explanation |
| Surprise redirects | User clicks one thing and ends up somewhere unexpected | Stay on the same page, show result inline |
| Endless scroll with footer | User can never reach the footer content | Use "load more" button, or put footer content in a sidebar |
| Auto-advance without warning | Form advances to next step before user is ready | Explicit "Next" button |
| Non-skippable onboarding | Forced tutorial before the user can do anything | Let users skip, offer help on demand |

## Checklist

- [ ] Every user action produces visible feedback
- [ ] Error messages are specific, actionable, and polite
- [ ] Empty states explain why and offer a next action
- [ ] Loading states use skeletons for content, spinners for short waits
- [ ] Destructive actions have undo or confirmation
- [ ] Interactive elements look interactive (affordance)
- [ ] Same actions look and work the same everywhere (consistency)
- [ ] Complex UIs use progressive disclosure (show less by default)
