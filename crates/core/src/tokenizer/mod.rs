pub trait Tokenizer: Send + Sync {
    fn count_tokens(&self, text: &str) -> usize;
}

pub struct TiktokenTokenizer {
    encoding: &'static tiktoken::CoreBpe,
}

impl TiktokenTokenizer {
    pub fn new(encoding: &'static tiktoken::CoreBpe) -> Self {
        Self { encoding }
    }

    pub fn for_model(model: &str) -> Option<Self> {
        tiktoken::encoding_for_model(model).map(|enc| Self { encoding: enc })
    }
}

impl Tokenizer for TiktokenTokenizer {
    fn count_tokens(&self, text: &str) -> usize {
        self.encoding.count(text)
    }
}

pub struct HeuristicTokenizer;

impl Tokenizer for HeuristicTokenizer {
    fn count_tokens(&self, text: &str) -> usize {
        crate::helpers::estimate_tokens(text)
    }
}

pub fn tokenizer_for_model(model: &str) -> Box<dyn Tokenizer> {
    TiktokenTokenizer::for_model(model)
        .map(|t| Box::new(t) as Box<dyn Tokenizer>)
        .unwrap_or_else(|| Box::new(HeuristicTokenizer))
}

pub fn offline_token_count(model: &str, text: &str) -> Option<usize> {
    TiktokenTokenizer::for_model(model).map(|t| t.count_tokens(text))
}
