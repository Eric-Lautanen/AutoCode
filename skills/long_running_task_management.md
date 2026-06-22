---
name: long-running-task-management
description: Use when a task is large enough to span multiple sessions, risk hitting context limits, or require tracking progress across many steps. Covers how to structure long work, checkpoint progress, write handoff notes, and resume without losing state. Load at the start of any task estimated to require significant back-and-forth or many file changes.
---

# Long-Running Task Management

## Overview

Large tasks — the kind that touch 10+ files, require 20+ steps, or span multiple sessions — fail without structure. The core principle: **every step should leave the project in a known, recoverable state.** If you lose context mid-task, you should be able to pick up exactly where you left off without re-reading everything.

## Estimating Task Size

Before starting, assess whether a task needs long-running management:

| Signal | Risk level | Action |
|--------|-----------|--------|
| Touches 1-2 files | Low | Just do it |
| Touches 3-5 files, clear steps | Medium | Write down the steps, then proceed |
| Touches 6+ files or has unknowns | High | Full checkpoint workflow |
| Spans multiple sessions | Critical | Handoff notes required |

**Ask yourself:** "If I lost all context right now, how much work would I lose?" If the answer is more than 15 minutes of re-reading, you need checkpoints.

## Checkpointing

### What a Checkpoint Looks Like
After each significant step, the project should be in this state:
- **Buildable**: The project compiles/builds without errors
- **Testable**: Existing tests pass (new tests for the step should also pass)
- **Committed** (if using git): Each checkpoint is a commit you can return to
- **Documented**: You know what was done and what's next

### Checkpoint Frequency
- After completing each step in your task decomposition
- Before any risky operation (large refactor, dependency upgrade, migration)
- Before switching context to a different sub-task
- When you've made significant progress and don't want to lose it

### What to Save at Each Checkpoint
1. The code changes (committed or at least saved)
2. A note of what was completed
3. A note of what's next
4. Any decisions made and why

## Progress Tracking

### Maintain a Running Task List
Use the `todo_list` tool to track progress. Update it after every step:

```
✅ Create User model
✅ Add migration for users table
✅ Implement registration endpoint
⬜ Add email verification
⬜ Write tests for registration flow
⬜ Update API documentation
```

### Mark Steps Complete as You Go
Don't batch updates — mark each step complete immediately after verification. This gives you an accurate picture of progress at any moment.

### Track Decisions
When you make a non-obvious decision during implementation, note it:
- "Chose cursor-based pagination over offset because the table has 2M+ rows"
- "Used bcrypt over argon2 because the deployment target has limited CPU"

These notes prevent you from re-litigating decisions you already made.

## Handoff Notes

When a task will span sessions, write handoff notes before ending the session.

### What to Include
```markdown
# Task: Add Email Verification to Registration

## Completed
- User model updated with `email_verified_at` field
- Migration created and tested
- Verification token generation implemented
- POST /auth/verify endpoint implemented

## In Progress
- Email sending integration (started, needs SendGrid template ID)

## Next Steps
1. Configure SendGrid template and complete email sending
2. Add rate limiting to /auth/verify endpoint
3. Write integration tests for the full flow
4. Update API documentation

## Decisions Made
- Tokens expire after 24 hours (configurable via EMAIL_TOKEN_TTL env var)
- Using HMAC-SHA256 for token generation (not JWT — simpler, no external deps)

## Blockers
- Need SendGrid API key from ops team
- Need to decide: should unverified users be able to reset their password?

## File Locations
- Auth service: src/services/auth.rs
- Verification handler: src/handlers/verify.rs
- Migration: migrations/014_add_email_verified_at.sql
```

### What NOT to Include
- Full file contents (the files exist — reference them, don't duplicate)
- Generic context ("this is a Rust project using Axum") — assume the next session can orient itself
- Every line of code you wrote — just the summary of what changed

## Resuming

When picking up a task from a previous session:

1. **Read the handoff notes.** Understand what was done and what's next.
2. **Verify the last checkpoint.** Build and test to confirm the project is in the state described.
3. **Confirm the next step before acting.** Don't assume — verify the starting point is what you expect.
4. **Re-read only what you need.** Don't re-read the entire codebase. Read the files mentioned in the handoff notes.

### If No Handoff Notes Exist
1. Check git log for recent commits — they tell you what was done
2. Check `git status` for uncommitted changes
3. Build and test to see the current state
4. Write handoff notes for the next person (which might be you)

## Avoiding Wasted Work

### Before Risky Operations
- **Commit your current work.** If the risky operation fails, you can `git reset --hard` back.
- **Create a branch.** If the experiment doesn't work, delete the branch.
- **Note what you're about to try.** If it fails, you'll know what didn't work.

### When to Save State
- Before a large refactoring step
- Before upgrading a dependency
- Before running a migration on real data
- Before an approach that might not work (spike)

## Communicating Blockers

### When to Stop and Ask
- You need information you don't have (API key, design decision, business rule)
- You've tried 3 approaches and none worked
- The task requires a decision that has tradeoffs you can't evaluate alone
- You've been stuck for more than 30 minutes

### What to Include When Asking
- What you're trying to do
- What you've tried and the results
- What options you see and their tradeoffs
- What you'd recommend and why

### When to Make a Decision and Proceed
- The decision is reversible (you can change it later)
- The tradeoffs are clear and one option is clearly better
- Waiting would block more work than the decision is worth
- Document the decision and the reasoning so it can be revisited

## Windows-Specific Task Management Notes

### Windows Service Long-Running Tasks
When implementing long-running tasks as Windows services:

```python
import win32serviceutil
import win32service
import win32event

class LongRunningService(win32serviceutil.ServiceFramework):
    _svc_name_ = "MyLongTask"
    _svc_display_name_ = "My Long Running Task"

    def __init__(self, args):
        win32serviceutil.ServiceFramework.__init__(self, args)
        self.stop_event = win32event.CreateEvent(None, 0, 0, None)

    def SvcStop(self):
        # Signal the service to stop
        win32event.SetEvent(self.stop_event)

    def SvcDoRun(self):
        # Main task loop
        while True:
            result = win32event.WaitForSingleObject(self.stop_event, 5000)
            if result == win32event.WAIT_OBJECT_0:
                break
            # Do work here
            self.run_task()

    def run_task(self):
        # Implement your long-running task
        pass
```

### Windows Task Scheduler
Use Windows Task Scheduler for periodic long-running tasks:

```powershell
# Create a scheduled task for a long-running script
$action = New-ScheduledTaskAction -Execute "python.exe" -Argument "C:\Scripts\long_task.py"
$trigger = New-ScheduledTaskTrigger -Daily -At 2am
$settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries
Register-ScheduledTask -TaskName "MyLongTask" -Action $action -Trigger $trigger -Settings $settings
```

### Windows Performance Monitoring
Monitor long-running tasks on Windows:

```powershell
# Get process info for a running task
Get-Process -Name "python" | Select-Object Name, Id, CPU, WorkingSet

# Check Windows Event Log for task events
Get-EventLog -LogName Application -Source "MyApp" -Newest 10
```

### Checkpointing on Windows
When checkpointing on Windows, consider:

- **File locking**: Windows locks open files; close handles before checkpointing
- **Path length**: Keep checkpoint paths under 260 characters
- **Permissions**: Ensure the service account can write checkpoint files

```python
import os
from pathlib import Path

def create_checkpoint(data, checkpoint_dir="checkpoints"):
    """Create a checkpoint file on Windows."""
    checkpoint_path = Path(checkpoint_dir) / f"checkpoint_{int(time.time())}.json"
    
    # Ensure directory exists
    checkpoint_path.parent.mkdir(parents=True, exist_ok=True)
    
    # Write atomically on Windows
    temp_path = checkpoint_path.with_suffix('.tmp')
    temp_path.write_text(json.dumps(data))
    temp_path.replace(checkpoint_path)  # Atomic on Windows with os.replace
```

## Anti-Patterns

- **Windows: Not handling service stop signals.** Windows services must respond to stop requests promptly.
- **Windows: Not using Task Scheduler for periodic tasks.** Reinventing scheduling is error-prone.
- **No checkpoints.** If you lose context, you lose hours of work.
- **Not committing between steps.** Uncommitted changes are one `git reset` away from oblivion.
- **Vague handoff notes.** "Did some stuff, more to do" is useless. Be specific about what's done and what's next.
- **Re-reading everything on resume.** You don't need to re-orient from scratch. Trust the handoff notes and verify the checkpoint.
- **Not tracking decisions.** Re-litigating decisions you already made wastes time and introduces inconsistency.
