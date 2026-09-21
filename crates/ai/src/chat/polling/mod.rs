use std::collections::HashMap;

use autocode_core::state::AppState;

use super::agents;
use super::completion::{check_auto_handoff, deliver_message, start_completion};
use super::runtime::{AgentOutcome, ChatRuntime};

mod shell;
mod stream;
mod tools;

use tools::commit_tool_results;

// -- Buffer size caps --------------------------------------------------------

const MAX_RESPONSE_SIZE: usize = 1024 * 1024; // 1MB cap
const MAX_REASONING_SIZE: usize = 512 * 1024; // 512KB cap

pub(super) fn append_to_pending(pending_response: &mut String, text: &str) {
    let remaining = MAX_RESPONSE_SIZE.saturating_sub(pending_response.len());
    if remaining > 0 {
        let end = text.floor_char_boundary(text.len().min(remaining));
        pending_response.push_str(&text[..end]);
    }
    if pending_response.len() >= MAX_RESPONSE_SIZE {
        pending_response.truncate(MAX_RESPONSE_SIZE);
        if !pending_response.ends_with("[Response truncated due to size limit]") {
            pending_response.push_str("\n[Response truncated due to size limit]");
        }
    }
}

pub(super) fn append_to_reasoning(reasoning_buf: &mut String, text: &str) {
    let remaining = MAX_REASONING_SIZE.saturating_sub(reasoning_buf.len());
    if remaining > 0 {
        let end = text.floor_char_boundary(text.len().min(remaining));
        reasoning_buf.push_str(&text[..end]);
    }
    if reasoning_buf.len() >= MAX_REASONING_SIZE {
        reasoning_buf.truncate(MAX_REASONING_SIZE);
        if !reasoning_buf.ends_with("[Reasoning truncated due to size limit]") {
            reasoning_buf.push_str("\n[Reasoning truncated due to size limit]");
        }
    }
}

// -- Per-frame update ----------------------------------------------------------

pub fn update_runtime(state: &mut AppState, runtime: &mut ChatRuntime) -> bool {
    let mut repaint = false;

    // Auto-handoff check runs BEFORE stream polling so it sees the same
    // state that was just rendered on screen. If it ran after poll_stream,
    // a sudden actual_tokens_used jump from the API response would trigger
    // handoff on the same frame — before the display can update — making
    // it appear to fire at a lower count than what the user sees.
    check_auto_handoff(state, runtime);

    repaint |= stream::poll_stream(state, runtime);
    repaint |= shell::poll_shell_tasks(state, runtime);
    repaint |= tools::poll_tool_results(state, runtime);
    repaint |= shell::poll_live_shell(state, runtime);
    repaint |= shell::poll_network(runtime);

    // Apply looping window pruning after tool results or text completions land.
    if let Some(sid) = runtime.active_session_id.as_deref() {
        super::looping::apply_looping_window(state, sid);
    }

    // Deferred start: fire completion the frame after send_message so the
    // user message bubble renders before the disk read + API call begins.
    if runtime.pending_start > 0 && !runtime.is_busy() {
        runtime.pending_start -= 1;
        if runtime.pending_start == 0 {
            start_completion(state, runtime);
        }
        return true;
    }

    // Retry backoff: non-blocking timer. Retries forever for transient errors,
    // only stopped by user interaction (stop button -> drain()).
    if let Some(after) = runtime.retry_after {
        repaint = true;
        let remaining = after
            .checked_duration_since(std::time::Instant::now())
            .unwrap_or_default();
        let remaining_secs = (remaining.as_millis() + 500) / 1000;
        // Live countdown -- only overwrite status if it's a rate-limit wait
        // (set by start_completion), not a retry backoff (set by error handler).
        if remaining_secs > 0 && runtime.status.starts_with("Rate limit") {
            runtime.status = format!(
                "Rate limit: waiting ~{}s before next request...",
                remaining_secs
            );
        }
        if remaining.is_zero()
            && runtime.stream_rx.is_none()
            && runtime.tool_rx.is_none()
            && runtime.live_shell_rx.is_none()
            && !runtime.agents_pending()
        {
            runtime.retry_after = None;
            runtime.status = "Starting request...".into();
            start_completion(state, runtime);
        }
    }

    // Queued user message: deliver the front of the queue the moment the turn
    // has fully settled, so a follow-up typed mid-turn becomes the NEXT user
    // turn. What "settled" means is `ChatRuntime::unsettled_reason` — one named
    // predicate, shared with the loop diagnostic, so a queue that is being held
    // open can never be an invisible mystery again.
    if runtime.turn_settled()
        && !runtime.queued_messages.is_empty()
        && let Some(sid) = runtime.active_session_id.clone()
        // Never deliver into a session that no longer exists (it is pruned
        // later in `update_all`); the message must not resurrect a dead tab.
        && state.sessions.iter().any(|s| s.id == sid)
    {
        let queued = runtime.queued_messages.remove(0);
        deliver_message(state, runtime, &sid, queued.text, queued.attachments);
        repaint = true;
    }

    repaint
}

/// Parent-side sub-agent settlement (D3): when every child of a batch is
/// terminal, commit one ToolResult per spawn_agent call and resume the
/// parent. Runs at update_all level because children live in the same
/// runtimes map the pump owns.
fn poll_agent_settlement(
    state: &mut AppState,
    runtimes: &mut HashMap<String, ChatRuntime>,
) -> bool {
    let parent_ids: Vec<String> = runtimes
        .iter()
        .filter(|(_, rt)| rt.agents_pending())
        .map(|(k, _)| k.clone())
        .collect();
    let repaint = !parent_ids.is_empty();
    for pid in parent_ids {
        // 1. Observe: which handles settled this frame?
        let mut settled: Vec<(usize, AgentOutcome)> = Vec::new();
        {
            let Some(parent) = runtimes.get(&pid) else {
                continue;
            };
            for (i, h) in parent.pending_agents.iter().enumerate() {
                if h.result.is_some() {
                    continue;
                }
                match runtimes.get(&h.agent_session_id) {
                    None => settled.push((
                        i,
                        AgentOutcome {
                            content: "[agent stopped unexpectedly]".to_string(),
                            is_error: true,
                        },
                    )),
                    Some(child) if agents::child_settled(child) => {
                        let outcome = agents::outcome_for_idle_child(state, &h.agent_session_id);
                        settled.push((i, outcome));
                    }
                    _ => {}
                }
            }
        }
        if settled.is_empty() {
            continue;
        }

        // 2. Fill handle results (child status was already persisted by
        //    outcome_for_idle_child / cancel paths).
        if let Some(parent) = runtimes.get_mut(&pid) {
            for (i, outcome) in &settled {
                if let Some(h) = parent.pending_agents.get_mut(*i) {
                    h.result = Some(outcome.clone());
                }
            }
        }

        // 3. Batch complete? Push one ToolResult per spawn_agent call in
        //    original order, clear the handles, and resume the parent.
        if runtimes.get(&pid).is_some_and(|rt| !rt.agents_pending()) {
            let done: Vec<(super::runtime::AgentHandle, AgentOutcome)> = runtimes
                .get(&pid)
                .map(|rt| {
                    rt.pending_agents
                        .iter()
                        .filter_map(|h| h.result.clone().map(|o| (h.clone(), o)))
                        .collect()
                })
                .unwrap_or_default();
            for (handle, outcome) in &done {
                agents::push_agent_result_msg(state, &pid, handle, outcome);
            }
            if let Some(parent) = runtimes.get_mut(&pid) {
                parent.pending_agents.clear();
            }
            // Continue: route any deferred normal-tool results through the
            // shared committer, otherwise start the next request directly.
            let snap = runtimes.get(&pid).map(|rt| {
                (
                    rt.tool_rx.is_none(),
                    rt.live_shell_rx.is_none(),
                    rt.pending_tool_remaining.is_empty(),
                    rt.stream_rx.is_none(),
                    rt.stopped_by_user,
                    rt.retry_after.is_none(),
                    rt.pending_tool_results.is_empty(),
                )
            });
            if let Some((
                tool_none,
                shell_none,
                rem_empty,
                stream_none,
                stopped,
                no_retry,
                no_stash,
            )) = snap
                && !stopped
                && stream_none
                && tool_none
                && shell_none
                && rem_empty
            {
                if !no_stash {
                    if let Some(rt) = runtimes.get_mut(&pid) {
                        commit_tool_results(state, rt);
                    }
                } else if no_retry && let Some(rt) = runtimes.get_mut(&pid) {
                    rt.status = "Agents finished -- continuing.".into();
                    start_completion(state, rt);
                }
                // A pending retry timer simply fires through update_runtime,
                // whose guard now sees no unsettled agents.
            }
        }
    }
    repaint
}

pub fn update_all(state: &mut AppState, runtimes: &mut HashMap<String, ChatRuntime>) -> bool {
    let mut repaint = false;
    // Publish runtime-owned session ids so core-side pruning (MAX_SESSIONS)
    // never evicts a session with a live runtime.
    state.runtime_sessions = runtimes.keys().cloned().collect();
    let keys: Vec<String> = runtimes.keys().cloned().collect();
    let mut rekeys: Vec<(String, String)> = Vec::new();
    for key in keys {
        if let Some(runtime) = runtimes.get_mut(&key) {
            repaint |= update_runtime(state, runtime);
            if let Some(ref new_sid) = runtime.active_session_id
                && new_sid != &key
            {
                rekeys.push((key.clone(), new_sid.clone()));
            }
        }
    }
    for (old_key, new_key) in rekeys {
        if let Some(runtime) = runtimes.remove(&old_key) {
            // Two runtimes must never claim one session. `insert` would clobber
            // the incumbent silently, and dropping a runtime is not free:
            // `drain()` is what kills its background shell processes and its
            // queued follow-ups would go with it. A handoff mints a fresh
            // session id so this should be unreachable — tear the displaced one
            // down rather than leak it, and say so, because a silent swap here
            // would look exactly like the queue vanishing on its own.
            if let Some(displaced) = runtimes.get_mut(&new_key) {
                eprintln!(
                    "[polling] runtime {old_key} is taking over {new_key}, which already had one -- draining the displaced runtime"
                );
                displaced.drain();
            }
            runtimes.insert(new_key, runtime);
        }
    }
    // Create runtimes for agents spawned during this frame's pumping.
    agents::create_queued_runtimes(state, runtimes);
    // Settle finished sub-agents and resume their parents.
    repaint |= poll_agent_settlement(state, runtimes);
    // Prune zombie runtimes for sessions deleted elsewhere (e.g. Settings UI).
    let valid_ids: std::collections::HashSet<String> =
        state.sessions.iter().map(|s| s.id.clone()).collect();
    runtimes.retain(|id, runtime| {
        if !valid_ids.contains(id) {
            runtime.drain();
            false
        } else {
            true
        }
    });
    repaint
}

#[cfg(test)]
mod tests {
    use super::super::completion::{
        cancel_queued_message, inject_queued_message_now, queue_message, take_queued_message,
    };
    use super::*;
    use autocode_core::state::Role;

    /// One active session (project-less, so nothing is written to disk) with a
    /// runtime registered for it — the shape `update_runtime` expects.
    fn fixture() -> (AppState, HashMap<String, ChatRuntime>, String) {
        let mut state = AppState::default();
        state.new_session_for_project(None);
        let sid = state.active_session_id.clone().expect("active session");
        let mut runtimes = HashMap::new();
        runtimes.insert(sid.clone(), ChatRuntime::default());
        (state, runtimes, sid)
    }

    /// User-message bodies in the session, in order. `push_to_session` stamps a
    /// `Time: ... UTC` line onto every user message, hence the `starts_with`
    /// assertions at the call sites.
    fn user_texts(state: &AppState, sid: &str) -> Vec<String> {
        state
            .sessions
            .iter()
            .find(|s| s.id == sid)
            .map(|s| {
                s.messages
                    .iter()
                    .filter(|m| m.role == Role::User)
                    .map(|m| m.content.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    fn pump(state: &mut AppState, runtimes: &mut HashMap<String, ChatRuntime>, sid: &str) {
        let runtime = runtimes.get_mut(sid).expect("runtime");
        update_runtime(state, runtime);
    }

    /// Model the request a delivered message armed actually going out: the
    /// deferred start fires, `start_completion` hands the request to the
    /// provider — which is exactly what clears `delivery_undispatched` — and the
    /// turn then streams on the returned channel.
    fn dispatch_a_turn(
        runtimes: &mut HashMap<String, ChatRuntime>,
        sid: &str,
    ) -> std::sync::mpsc::Sender<crate::provider::ProviderEvent> {
        use crate::provider::CompletionStream;
        use std::sync::Arc;
        use std::sync::atomic::AtomicBool;

        let (tx, rx) = std::sync::mpsc::channel();
        let rt = runtimes.get_mut(sid).expect("runtime");
        rt.pending_start = 0;
        rt.delivery_undispatched = false;
        rt.stream_rx = Some(CompletionStream::new(rx, Arc::new(AtomicBool::new(false))));
        tx
    }

    /// Simulate a turn that is genuinely in flight: the runtime is waiting on a
    /// sub-agent of its current batch, so there is real running work to protect.
    /// (A `retry_after` timer would NOT do: that is a *scheduled* wait, which a
    /// queued message is allowed to pre-empt — see
    /// `a_scheduled_retry_does_not_hold_the_queue`.)
    fn mark_busy(runtimes: &mut HashMap<String, ChatRuntime>, sid: &str) {
        let rt = runtimes.get_mut(sid).expect("runtime");
        rt.pending_agents.push(super::super::runtime::AgentHandle {
            tool_call_id: "call_agent".into(),
            agent_session_id: "agent-session".into(),
            started: std::time::Instant::now(),
            result: None,
        });
    }

    #[test]
    fn queued_message_is_delivered_once_the_turn_settles() {
        let (mut state, mut runtimes, sid) = fixture();
        let before = user_texts(&state, &sid).len();

        queue_message(&mut state, &mut runtimes, "follow up".into(), Vec::new());
        assert_eq!(runtimes[&sid].queued_messages.len(), 1);
        assert_eq!(user_texts(&state, &sid).len(), before);

        // The runtime is idle, so the very next frame is "the turn finished".
        pump(&mut state, &mut runtimes, &sid);

        assert!(runtimes[&sid].queued_messages.is_empty());
        assert_eq!(
            runtimes[&sid].pending_start, 2,
            "delivery arms the deferred completion like a typed send"
        );
        let texts = user_texts(&state, &sid);
        assert_eq!(texts.len(), before + 1);
        assert!(texts.last().unwrap().starts_with("follow up"));
    }

    #[test]
    fn queued_message_waits_while_a_turn_is_in_flight() {
        let (mut state, mut runtimes, sid) = fixture();
        mark_busy(&mut runtimes, &sid);
        queue_message(&mut state, &mut runtimes, "follow up".into(), Vec::new());

        pump(&mut state, &mut runtimes, &sid);

        assert_eq!(
            runtimes[&sid].queued_messages.len(),
            1,
            "a busy runtime must not have its turn interrupted by the queue"
        );
        assert!(user_texts(&state, &sid).is_empty());
        assert_eq!(runtimes[&sid].pending_start, 0);
    }

    /// A rate-limit wait or an error backoff parks the *next* request behind
    /// `retry_after`. Nothing is running, so the queued follow-up goes out now
    /// and cancels that wait. This is the state a slow provider leaves the
    /// runtime in for most of its wall time — counting it as "busy" starved the
    /// queue for minutes at a stretch while the UI showed the Stop button.
    #[test]
    fn a_scheduled_retry_does_not_hold_the_queue() {
        let (mut state, mut runtimes, sid) = fixture();
        runtimes.get_mut(&sid).unwrap().retry_after =
            Some(std::time::Instant::now() + std::time::Duration::from_secs(60));
        assert!(
            runtimes[&sid].is_busy(),
            "the UI still treats a scheduled retry as the AI working"
        );
        assert!(
            runtimes[&sid].turn_settled(),
            "...but nothing is actually running, so the queue may go out"
        );
        queue_message(
            &mut state,
            &mut runtimes,
            "never mind, do X".into(),
            Vec::new(),
        );

        pump(&mut state, &mut runtimes, &sid);

        let rt = &runtimes[&sid];
        assert!(rt.queued_messages.is_empty(), "the queue drained");
        assert!(
            rt.retry_after.is_none(),
            "the injected message cancels the pending retry"
        );
        assert_eq!(rt.pending_start, 2, "and arms the deferred start");
        let texts = user_texts(&state, &sid);
        assert_eq!(texts.len(), 1);
        assert!(texts[0].starts_with("never mind, do X"));
    }

    /// The queue advances one *turn* at a time, not one "thing that looks
    /// settled" at a time. A delivery pre-empting a scheduled wait is exactly
    /// what unsticks a queue behind a rate-limited provider, but the request it
    /// arms can then be parked on that same timer — and that parked request must
    /// not be mistaken for another finished turn, or a whole queue drains into
    /// one request and the model gets several user messages in a row with no
    /// reply between them.
    #[test]
    fn a_rate_limited_provider_cannot_swallow_the_whole_queue() {
        let (mut state, mut runtimes, sid) = fixture();
        queue_message(&mut state, &mut runtimes, "first".into(), Vec::new());
        queue_message(&mut state, &mut runtimes, "second".into(), Vec::new());

        // The turn they were typed into ends: the head goes out.
        pump(&mut state, &mut runtimes, &sid);
        assert_eq!(runtimes[&sid].queued_messages.len(), 1);

        // Its deferred start fires and the provider's rate limiter parks the
        // request: a scheduled wait, nothing running.
        {
            let rt = runtimes.get_mut(&sid).expect("runtime");
            rt.pending_start = 0;
            rt.retry_after = Some(std::time::Instant::now() + std::time::Duration::from_secs(60));
        }
        for _ in 0..5 {
            pump(&mut state, &mut runtimes, &sid);
        }

        assert_eq!(
            runtimes[&sid].queued_messages.len(),
            1,
            "the second follow-up waits for the first one's turn to run"
        );
        assert_eq!(
            user_texts(&state, &sid).len(),
            1,
            "only the delivered message reached the transcript so far"
        );

        // Once that request does go out and its turn finishes, the next one goes.
        let tx_turn = dispatch_a_turn(&mut runtimes, &sid);
        tx_turn
            .send(crate::provider::ProviderEvent::Delta(
                "answered first".into(),
            ))
            .unwrap();
        tx_turn
            .send(crate::provider::ProviderEvent::Done {
                prompt_tokens: 9,
                completion_tokens: 4,
                finish_reason: Some("stop".into()),
            })
            .unwrap();
        pump(&mut state, &mut runtimes, &sid);

        assert!(runtimes[&sid].queued_messages.is_empty());
        let texts = user_texts(&state, &sid);
        assert_eq!(texts.len(), 2);
        assert!(texts[1].starts_with("second"));
    }

    #[test]
    fn queued_message_waits_for_a_buffered_response_to_commit() {
        // A tool batch leaves the stream momentarily idle between the
        // assistant's tool_calls and their results. The buffered response is the
        // marker that the turn is not over yet.
        let (mut state, mut runtimes, sid) = fixture();
        runtimes.get_mut(&sid).unwrap().pending_response = "half an answer".into();
        queue_message(&mut state, &mut runtimes, "follow up".into(), Vec::new());

        pump(&mut state, &mut runtimes, &sid);

        assert_eq!(runtimes[&sid].queued_messages.len(), 1);
        assert!(user_texts(&state, &sid).is_empty());
    }

    #[test]
    fn queue_waiting_on_a_deleted_session_is_never_delivered() {
        let (mut state, mut runtimes, sid) = fixture();
        queue_message(&mut state, &mut runtimes, "follow up".into(), Vec::new());
        state.sessions.clear();
        state.active_session_id = None;

        pump(&mut state, &mut runtimes, &sid);

        assert_eq!(runtimes[&sid].queued_messages.len(), 1);
    }

    #[test]
    fn inject_now_hits_stop_then_delivers_right_away() {
        use crate::provider::CompletionStream;
        use std::sync::Arc;
        use std::sync::atomic::AtomicBool;

        let (mut state, mut runtimes, sid) = fixture();
        // A real in-flight stream, so "the interrupt actually stops something"
        // is asserted against the stream handle the provider worker owns.
        let (_tx, rx) = std::sync::mpsc::channel();
        {
            let rt = runtimes.get_mut(&sid).expect("runtime");
            rt.active_session_id = Some(sid.clone());
            rt.stream_rx = Some(CompletionStream::new(rx, Arc::new(AtomicBool::new(false))));
        }
        assert!(runtimes[&sid].is_busy());
        queue_message(&mut state, &mut runtimes, "urgent".into(), Vec::new());

        inject_queued_message_now(&mut state, &mut runtimes, &sid, 0);

        let rt = &runtimes[&sid];
        assert!(rt.queued_messages.is_empty(), "the message left the queue");
        assert!(
            rt.stream_rx.is_none(),
            "the in-flight request was torn down exactly like Stop"
        );
        assert!(rt.stopped_by_user, "the interrupt is recorded like a Stop");
        assert!(!rt.is_busy(), "nothing is left in flight");
        assert_eq!(
            rt.pending_start, 2,
            "the injected message arms a deferred start, like a typed send"
        );
        let texts = user_texts(&state, &sid);
        assert_eq!(texts.len(), 1);
        assert!(texts[0].starts_with("urgent"));
    }

    #[test]
    fn inject_now_on_an_idle_runtime_stops_nothing() {
        let (mut state, mut runtimes, sid) = fixture();
        queue_message(&mut state, &mut runtimes, "urgent".into(), Vec::new());

        inject_queued_message_now(&mut state, &mut runtimes, &sid, 0);

        let rt = &runtimes[&sid];
        assert!(!rt.stopped_by_user);
        assert_eq!(rt.pending_start, 2);
        assert!(user_texts(&state, &sid)[0].starts_with("urgent"));
    }

    /// Injecting into a session that has been deleted must not eat the message:
    /// the delivery would push it into nothing. It stays queued (and visible)
    /// instead, matching what the auto-delivery guard refuses.
    #[test]
    fn inject_now_into_a_deleted_session_keeps_the_message_queued() {
        let (mut state, mut runtimes, sid) = fixture();
        queue_message(&mut state, &mut runtimes, "still mine".into(), Vec::new());
        state.sessions.clear();

        inject_queued_message_now(&mut state, &mut runtimes, &sid, 0);

        assert_eq!(runtimes[&sid].queued_messages.len(), 1);
        assert_eq!(runtimes[&sid].pending_start, 0);
        assert!(user_texts(&state, &sid).is_empty());
    }

    #[test]
    fn inject_now_ignores_an_out_of_range_index() {
        let (mut state, mut runtimes, sid) = fixture();
        queue_message(&mut state, &mut runtimes, "only".into(), Vec::new());

        inject_queued_message_now(&mut state, &mut runtimes, &sid, 7);

        assert_eq!(runtimes[&sid].queued_messages.len(), 1);
        assert_eq!(runtimes[&sid].pending_start, 0);
        assert!(user_texts(&state, &sid).is_empty());
    }

    #[test]
    fn cancel_and_take_address_the_queue_by_index() {
        let (mut state, mut runtimes, sid) = fixture();
        for text in ["first", "second", "third"] {
            queue_message(&mut state, &mut runtimes, text.into(), Vec::new());
        }

        cancel_queued_message(&mut runtimes, &sid, 1);
        let remaining: Vec<String> = runtimes[&sid]
            .queued_messages
            .iter()
            .map(|q| q.text.clone())
            .collect();
        assert_eq!(remaining, ["first", "third"]);

        // `take` is what the click-to-edit affordance uses to pull a message
        // back into the input box.
        let taken = take_queued_message(&mut runtimes, &sid, 0).expect("taken");
        assert_eq!(taken.text, "first");
        assert_eq!(runtimes[&sid].queued_messages[0].text, "third");

        // Out-of-range and unknown-session calls are no-ops, not panics.
        assert!(take_queued_message(&mut runtimes, &sid, 9).is_none());
        assert!(take_queued_message(&mut runtimes, "no-such-session", 0).is_none());
        cancel_queued_message(&mut runtimes, "no-such-session", 0);
        assert_eq!(runtimes[&sid].queued_messages.len(), 1);
    }

    #[test]
    fn queueing_blank_text_is_ignored() {
        let (mut state, mut runtimes, sid) = fixture();

        queue_message(&mut state, &mut runtimes, "   \n ".into(), Vec::new());

        assert!(runtimes[&sid].queued_messages.is_empty());
    }

    /// Drives a whole turn through the real polling loop: the model streams a
    /// final answer, the provider reports Done, the assistant message commits —
    /// and only then does the queued follow-up go out. This is the end-to-end
    /// check that the auto-delivery guard matches the state a *finished* turn
    /// actually leaves behind (no leftover tool batch, buffered text or agent
    /// handle pins the queue open forever).
    #[test]
    fn a_queued_message_goes_out_when_the_ai_finishes_its_turn() {
        use crate::provider::{CompletionStream, ProviderEvent};
        use std::sync::Arc;
        use std::sync::atomic::AtomicBool;

        let (mut state, mut runtimes, sid) = fixture();
        // A turn is streaming on this session.
        let (tx, rx) = std::sync::mpsc::channel();
        {
            let rt = runtimes.get_mut(&sid).expect("runtime");
            rt.active_session_id = Some(sid.clone());
            rt.stream_rx = Some(CompletionStream::new(rx, Arc::new(AtomicBool::new(false))));
        }
        assert!(runtimes[&sid].is_busy());

        // The user types a follow-up and hits queue while the model is writing.
        queue_message(&mut state, &mut runtimes, "then do X".into(), Vec::new());

        // The model finishes: text, then Done.
        tx.send(ProviderEvent::Delta("All done.".into())).unwrap();
        tx.send(ProviderEvent::Done {
            prompt_tokens: 12,
            completion_tokens: 3,
            finish_reason: Some("stop".into()),
        })
        .unwrap();
        pump(&mut state, &mut runtimes, &sid);

        // The answer committed...
        let assistant: Vec<String> = state.sessions[0]
            .messages
            .iter()
            .filter(|m| m.role == Role::Assistant)
            .map(|m| m.content.clone())
            .collect();
        assert_eq!(assistant.len(), 1);
        assert!(assistant[0].starts_with("All done."));

        // ...and the queued follow-up became the next user turn.
        let rt = &runtimes[&sid];
        assert!(!rt.is_busy(), "the finished turn must not still block");
        assert!(rt.queued_messages.is_empty(), "the queue was drained");
        assert_eq!(rt.pending_start, 2, "the follow-up is armed to be sent");
        let texts = user_texts(&state, &sid);
        assert_eq!(texts.len(), 1);
        assert!(texts[0].starts_with("then do X"));

        drop(tx);
    }

    /// Several follow-ups can be queued mid-turn, and they arrive in order, one
    /// per turn: the head goes out when the current turn ends and the rest wait
    /// for the turn that message itself starts. That ordering is what makes
    /// "then do X" / "then Y" work — the model always answers before the next
    /// instruction lands, exactly as if the user had typed them one at a time.
    #[test]
    fn queued_messages_go_out_in_order_one_per_turn() {
        use crate::provider::{CompletionStream, ProviderEvent};
        use std::sync::Arc;
        use std::sync::atomic::AtomicBool;

        let (mut state, mut runtimes, sid) = fixture();

        // A turn is streaming, and the user queues three follow-ups into it.
        let (tx, rx) = std::sync::mpsc::channel();
        {
            let rt = runtimes.get_mut(&sid).expect("runtime");
            rt.active_session_id = Some(sid.clone());
            rt.stream_rx = Some(CompletionStream::new(rx, Arc::new(AtomicBool::new(false))));
        }
        for text in ["first", "second", "third"] {
            queue_message(&mut state, &mut runtimes, text.into(), Vec::new());
        }
        assert_eq!(runtimes[&sid].queued_messages.len(), 3);

        // The turn finishes: the head goes out, the other two stay.
        let finish = |tx: &std::sync::mpsc::Sender<ProviderEvent>, answer: &str| {
            tx.send(ProviderEvent::Delta(answer.into())).unwrap();
            tx.send(ProviderEvent::Done {
                prompt_tokens: 9,
                completion_tokens: 4,
                finish_reason: Some("stop".into()),
            })
            .unwrap();
        };
        finish(&tx, "answered first");
        pump(&mut state, &mut runtimes, &sid);
        assert_eq!(runtimes[&sid].queued_messages.len(), 2);

        // The remaining two each wait for the turn the message before them
        // starts: nothing moves while that turn is running.
        for (answer, remaining) in [("answered second", 1), ("answered third", 0)] {
            let tx_turn = dispatch_a_turn(&mut runtimes, &sid);
            pump(&mut state, &mut runtimes, &sid);
            assert_eq!(
                runtimes[&sid].queued_messages.len(),
                remaining + 1,
                "a queued message waits for the turn it follows"
            );
            finish(&tx_turn, answer);
            pump(&mut state, &mut runtimes, &sid);
            assert_eq!(runtimes[&sid].queued_messages.len(), remaining);
        }

        let texts = user_texts(&state, &sid);
        let order: Vec<&str> = texts
            .iter()
            .map(|t| t.lines().next().unwrap_or(""))
            .collect();
        assert_eq!(order, ["first", "second", "third"]);
        assert!(runtimes[&sid].queued_messages.is_empty());
    }

    /// The shape a user actually hits: the model calls a tool, the batch runs
    /// on a worker thread, the results commit, and the model answers. The
    /// queued follow-up must go out once THAT whole exchange is over — a tool
    /// batch passing through the stream leaves several "in-flight" markers
    /// behind one at a time, and any one of them left set would hold the queue
    /// open forever.
    #[test]
    fn a_queued_message_survives_a_tool_calling_turn() {
        use crate::provider::{CompletionStream, ProviderEvent, ToolCall};
        use std::sync::Arc;
        use std::sync::atomic::AtomicBool;

        let (mut state, mut runtimes, sid) = fixture();
        let (tx, rx) = std::sync::mpsc::channel();
        {
            let rt = runtimes.get_mut(&sid).expect("runtime");
            rt.active_session_id = Some(sid.clone());
            rt.stream_rx = Some(CompletionStream::new(rx, Arc::new(AtomicBool::new(false))));
        }
        queue_message(
            &mut state,
            &mut runtimes,
            "after the tools".into(),
            Vec::new(),
        );

        // Turn 1: the model asks for one tool and stops.
        tx.send(ProviderEvent::ToolCall(ToolCall {
            id: "call_1".into(),
            name: "read_file".into(),
            arguments: r#"{"path":"definitely_not_here.txt"}"#.into(),
        }))
        .unwrap();
        tx.send(ProviderEvent::Done {
            prompt_tokens: 20,
            completion_tokens: 4,
            finish_reason: Some("tool_calls".into()),
        })
        .unwrap();
        pump(&mut state, &mut runtimes, &sid);
        assert!(
            runtimes[&sid].tool_rx.is_some(),
            "the batch was dispatched to a worker"
        );
        assert_eq!(
            runtimes[&sid].queued_messages.len(),
            1,
            "a live tool batch must hold the queue"
        );

        // Pump until the batch lands and its results commit.
        for _ in 0..400 {
            if runtimes[&sid].tool_rx.is_none() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
            pump(&mut state, &mut runtimes, &sid);
        }
        assert!(runtimes[&sid].tool_rx.is_none(), "the tool batch finished");
        // One more frame: the results committed on the previous pump and the
        // runtime now has no turn in flight.
        pump(&mut state, &mut runtimes, &sid);

        let rt = &runtimes[&sid];
        // No leftover marker may pin the queue open once the batch is done. Each
        // of these is a window between an assistant's `tool_calls` message and
        // its results, where a user message would orphan the batch.
        for (field, blocked) in [
            ("pending_tool_results", !rt.pending_tool_results.is_empty()),
            (
                "pending_tool_remaining",
                !rt.pending_tool_remaining.is_empty(),
            ),
            ("pending_tool_calls", !rt.pending_tool_calls.is_empty()),
            (
                "assistant_tool_calls_json",
                rt.assistant_tool_calls_json.is_some(),
            ),
            ("live_batch", !rt.live_batch.is_empty()),
            ("live_tool_call", rt.live_tool_call.is_some()),
            ("pending_response", !rt.pending_response.is_empty()),
            ("reasoning_buf", !rt.reasoning_buf.is_empty()),
            ("handoff_in_progress", rt.handoff_in_progress),
        ] {
            assert!(
                !blocked,
                "{field} still pins the queue open after the batch"
            );
        }
        // The only reason left to hold the queue is the delivered message's own
        // deferred start.
        assert_eq!(rt.unsettled_reason(), Some("deferred-start"));
        assert!(
            rt.queued_messages.is_empty(),
            "the follow-up went out once the tool exchange settled"
        );
        // The delivery happened on the frame the results committed; the pumps
        // after it may already have burned a tick of the deferred start.
        assert!(
            rt.pending_start > 0,
            "delivery arms the deferred start like a typed send"
        );
        let texts = user_texts(&state, &sid);
        assert!(texts.iter().any(|t| t.starts_with("after the tools")));

        drop(tx);
    }
}
