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
        // Try exact model match first
        if let Some(enc) = tiktoken::encoding_for_model(model) {
            return Some(Self { encoding: enc });
        }
        // Fall back to encoding based on model family
        Self::for_model_family(model)
    }

    fn for_model_family(model: &str) -> Option<Self> {
        let model_lower = model.to_lowercase();

        // tiktoken::get_encoding returns Option<&CoreBpe>
        let try_encoding =
            |name: &str| -> Option<&'static tiktoken::CoreBpe> { tiktoken::get_encoding(name) };

        // OpenAI GPT-4o, GPT-4o-mini, o1/o3 series -> o200k_base
        if model_lower.contains("gpt-4o")
            || model_lower.contains("o1")
            || model_lower.contains("o3")
        {
            return try_encoding("o200k_base").map(|enc| Self { encoding: enc });
        }

        // GPT-4, GPT-3.5, text-embedding-3, text-embedding-ada-002 -> cl100k_base
        if model_lower.contains("gpt-4")
            || model_lower.contains("gpt-3.5")
            || model_lower.contains("text-embedding-3")
            || model_lower.contains("text-embedding-ada")
        {
            return try_encoding("cl100k_base").map(|enc| Self { encoding: enc });
        }

        // Codex, code-davinci, older code models -> p50k_base
        if model_lower.contains("codex")
            || model_lower.contains("code-davinci")
            || model_lower.contains("code-cushman")
        {
            return try_encoding("p50k_base").map(|enc| Self { encoding: enc });
        }

        // GPT-2, older models -> gpt2 (r50k_base)
        if model_lower.contains("gpt-2") || model_lower.contains("gpt2") {
            return try_encoding("gpt2").map(|enc| Self { encoding: enc });
        }

        // Anthropic Claude models - no direct tiktoken support, but cl100k_base is closest
        if model_lower.contains("claude") {
            return try_encoding("cl100k_base").map(|enc| Self { encoding: enc });
        }

        // Google Gemini - no direct support, cl100k_base is reasonable approximation
        if model_lower.contains("gemini") {
            return try_encoding("cl100k_base").map(|enc| Self { encoding: enc });
        }

        // Meta Llama - no direct support, cl100k_base is reasonable
        if model_lower.contains("llama") || model_lower.contains("llama-") {
            return try_encoding("cl100k_base").map(|enc| Self { encoding: enc });
        }

        // Mistral - cl100k_base approximation
        if model_lower.contains("mistral") || model_lower.contains("mixtral") {
            return try_encoding("cl100k_base").map(|enc| Self { encoding: enc });
        }

        // Default fallback for unknown models - use cl100k_base as most common
        try_encoding("cl100k_base").map(|enc| Self { encoding: enc })
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
