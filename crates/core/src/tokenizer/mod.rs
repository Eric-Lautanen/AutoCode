pub trait Tokenizer: Send + Sync {
    fn count_tokens(&self, text: &str) -> usize;
}

pub struct HeuristicTokenizer;

impl Tokenizer for HeuristicTokenizer {
    fn count_tokens(&self, text: &str) -> usize {
        crate::helpers::estimate_tokens(text)
    }
}

pub fn tokenizer_for_model(_model: &str) -> Box<dyn Tokenizer> {
    Box::new(HeuristicTokenizer)
}

pub fn offline_token_count(_model: &str, text: &str) -> Option<usize> {
    Some(crate::helpers::estimate_tokens(text))
}
