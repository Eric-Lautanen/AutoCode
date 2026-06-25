pub fn is_incomplete_task_response(text: &str) -> bool {
    let lower = text.to_lowercase();
    let signals_continuation = [
        "let me read the rest",
        "let me quickly read",
        "let me continue",
        "i'll continue",
        "i'll now read",
        "i'll read the rest",
        "let me now read",
        "continuing with",
        "moving on to",
        "next, i'll read",
        "reading the remaining",
        "let me proceed",
    ];
    signals_continuation.iter().any(|s| lower.contains(s))
}
