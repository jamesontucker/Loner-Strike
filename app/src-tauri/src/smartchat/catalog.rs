//! What Smarter chat can download: the models (pinned to one Hugging Face revision and
//! checked by sha256) and llama.cpp's llama-server (pinned to one GitHub release).
//! Edit here to add, swap or update a model; nothing else needs to change.

/// How a model is asked for a reply.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PromptMode {
    /// A base model continues a fake chat log that ends in "<bot>:" (/completion).
    Continue,
    /// A chat model gets a short system prompt and a few example turns (/v1/chat/completions).
    Chat,
}

impl PromptMode {
    pub fn key(self) -> &'static str {
        match self {
            PromptMode::Continue => "continue",
            PromptMode::Chat => "chat",
        }
    }
}

pub struct Model {
    pub id: &'static str,
    pub name: &'static str,
    /// Bytes on disk (the GGUF file)
    pub size: u64,
    /// "Fast", "Medium" or "Slower", measured on an M3 Max and guessed for a 4-core PC
    pub speed: &'static str,
    pub good_at: &'static str,
    pub credit: &'static str,
    /// The model's page, for the credit link
    pub page: &'static str,
    /// .../resolve/<commit sha>/<file>.gguf
    pub url: &'static str,
    pub sha256: &'static str,
    pub mode: PromptMode,
}

pub const MODELS: &[Model] = &[
    Model {
        id: "llama32-1b",
        name: "Llama 3.2 1B",
        size: 807_690_208,
        speed: "Fast",
        good_at: "Short, messy pub replies. The quickest and lightest.",
        credit: "Llama 3.2 by Meta, Llama 3.2 Community License. GGUF by QuantFactory.",
        page: "https://huggingface.co/QuantFactory/Llama-3.2-1B-GGUF",
        url: "https://huggingface.co/QuantFactory/Llama-3.2-1B-GGUF/resolve/983f8cd1e4a8227322945f7f4a9aa97346ff1f65/Llama-3.2-1B.Q4_K_M.gguf",
        sha256: "ac81ee4bf5b82d94f380a87fd48560254b970107a94b6f228b2a36991853af60",
        mode: PromptMode::Continue,
    },
    Model {
        id: "micae-3b",
        name: "Discord Micae 3B",
        size: 2_019_374_496,
        speed: "Medium",
        good_at: "Casual back-and-forth. Trained on Discord chats.",
        credit: "Discord-Micae-Hermes-3-3B by mookiezi, on Hermes 3 (Nous Research) and Llama 3.2 by Meta, Llama 3 Community License. GGUF by mradermacher.",
        page: "https://huggingface.co/mradermacher/Discord-Micae-Hermes-3-3B-GGUF",
        url: "https://huggingface.co/mradermacher/Discord-Micae-Hermes-3-3B-GGUF/resolve/72b4f0da224c8b41544f2171b3f9b10d8e3ee054/Discord-Micae-Hermes-3-3B.Q4_K_M.gguf",
        sha256: "e22badc866240ac51a44c40fdbb9c98a0c5e14b967e3752906f1b3c932a7aaeb",
        mode: PromptMode::Chat,
    },
    Model {
        id: "gemma4npc-e2b",
        name: "Gemma4NPC E2B",
        size: 3_427_863_872,
        speed: "Slower",
        good_at: "Staying in character. Made for game characters.",
        credit: "Gemma4NPC-E2B by chimbiwide (npcLM), on Gemma 4 by Google, Apache 2.0.",
        page: "https://huggingface.co/chimbiwide/Gemma4NPC-E2B-GGUF",
        url: "https://huggingface.co/chimbiwide/Gemma4NPC-E2B-GGUF/resolve/b8e57bca1c8e5455178c16cd9554b2f75bb0c9fc/Gemma4NPC-E2B-Q4_K_M.gguf",
        sha256: "d808fcac6160f69139cca6583e3d66470f2b21fb3b6a1895ee01bf47a29f721e",
        mode: PromptMode::Chat,
    },
];

pub fn model(id: &str) -> Option<&'static Model> {
    MODELS.iter().find(|m| m.id == id)
}

/// llama.cpp's server, one build per platform, all from the same release.
pub const LLAMA_CPP_TAG: &str = "b11722";

pub struct Runtime {
    pub os: &'static str,
    pub arch: &'static str,
    pub url: &'static str,
    pub sha256: &'static str,
    pub size: u64,
}

pub const RUNTIMES: &[Runtime] = &[
    Runtime {
        os: "macos",
        arch: "aarch64",
        url: "https://github.com/ggml-org/llama.cpp/releases/download/b11722/llama-b11722-bin-macos-arm64.tar.gz",
        sha256: "2c570f3ed66b84ee40cd7581a2258018dd4da0e3d78c41e9486c9dbab589778e",
        size: 12_292_666,
    },
    Runtime {
        os: "macos",
        arch: "x86_64",
        url: "https://github.com/ggml-org/llama.cpp/releases/download/b11722/llama-b11722-bin-macos-x64.tar.gz",
        sha256: "8c63af67b9981212a1371bcbaa47ee3fabc5a769267dc6ff5edb32967a99bb0c",
        size: 11_781_401,
    },
    Runtime {
        os: "windows",
        arch: "x86_64",
        url: "https://github.com/ggml-org/llama.cpp/releases/download/b11722/llama-b11722-bin-win-cpu-x64.zip",
        sha256: "0449f7320b6019519974c3d02596edb84e7f18af9f04aaa6972ba0a960f4b8e5",
        size: 19_666_948,
    },
];

/// The build for this computer, if there is one.
pub fn runtime() -> Option<&'static Runtime> {
    RUNTIMES.iter().find(|r| r.os == std::env::consts::OS && r.arch == std::env::consts::ARCH)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_hex(s: &str, n: usize) -> bool {
        s.len() == n && s.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    }

    #[test]
    fn models_are_pinned() {
        let mut ids = std::collections::HashSet::new();
        for m in MODELS {
            assert!(ids.insert(m.id), "duplicate id {}", m.id);
            assert!(m.id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'), "{}", m.id);
            assert!(is_hex(m.sha256, 64), "{} sha256", m.id);
            assert!(m.size > 100_000_000 && m.size < 4_000_000_000, "{} size", m.id);
            // a revision, never a branch, so the file can't change under the checksum
            let rest = m.url.strip_prefix("https://huggingface.co/").expect("Hugging Face URL");
            let parts: Vec<&str> = rest.split('/').collect();
            assert_eq!(parts.len(), 5, "{}", m.url);
            assert_eq!(parts[2], "resolve", "{}", m.url);
            assert!(is_hex(parts[3], 40), "{} isn't pinned to a commit", m.url);
            assert!(parts[4].ends_with(".gguf"), "{}", m.url);
            assert!(m.page.starts_with(&format!("https://huggingface.co/{}/{}", parts[0], parts[1])));
            for s in [m.name, m.good_at, m.credit, m.speed] {
                assert!(!s.is_empty() && !s.contains('!') && !s.contains('\u{2014}'), "{s}");
            }
            assert!(["Fast", "Medium", "Slower"].contains(&m.speed));
        }
        assert!(model("llama32-1b").is_some_and(|m| m.mode == PromptMode::Continue));
    }

    #[test]
    fn runtimes_are_pinned() {
        for r in RUNTIMES {
            assert!(is_hex(r.sha256, 64));
            assert!(r.url.starts_with(&format!("https://github.com/ggml-org/llama.cpp/releases/download/{LLAMA_CPP_TAG}/llama-{LLAMA_CPP_TAG}-bin-")), "{}", r.url);
        }
        for (os, arch) in [("macos", "aarch64"), ("macos", "x86_64"), ("windows", "x86_64")] {
            assert!(RUNTIMES.iter().any(|r| r.os == os && r.arch == arch), "{os} {arch}");
        }
    }
}
