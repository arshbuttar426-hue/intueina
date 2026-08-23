use as_not_as_protocol::scorer::{PreparedScoring, Scorer};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;

// ── stopwords ──────────────────────────────────────────────

const STOPWORDS: &[&str] = &[
    "i", "me", "my", "myself", "we", "our", "ours", "ourselves",
    "you", "your", "yours", "yourself", "yourselves", "he", "him",
    "his", "himself", "she", "her", "hers", "herself", "it", "its",
    "itself", "they", "them", "their", "theirs", "themselves",
    "what", "which", "who", "whom", "this", "that", "these", "those",
    "am", "is", "are", "was", "were", "be", "been", "being",
    "have", "has", "had", "having", "do", "does", "did", "doing",
    "a", "an", "the", "and", "but", "if", "or", "because", "as",
    "until", "while", "of", "at", "by", "for", "with", "about",
    "against", "between", "into", "through", "during", "before",
    "after", "above", "below", "to", "from", "up", "down", "in",
    "out", "on", "off", "over", "under", "again", "further",
    "then", "once", "here", "there", "when", "where", "why",
    "how", "all", "any", "both", "each", "few", "more", "most",
    "other", "some", "such", "no", "nor", "not", "only", "own",
    "same", "so", "than", "too", "very", "just", "don", "now",
    "dont", "isnt", "doesnt", "didnt", "wont", "wouldnt",
    "cant", "couldnt", "shouldnt", "im", "ive", "youre",
    "theyre", "its", "thatll", "wouldve", "couldve", "shouldve",
];

fn is_stopword(w: &str) -> bool {
    STOPWORDS.contains(&w)
}

fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|w| w.len() >= 2)
        .map(|s| s.to_string())
        .collect()
}

fn filter_content_words(words: &[String]) -> Vec<String> {
    words.iter().filter(|w| !is_stopword(w)).cloned().collect()
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric()
}

fn contains_word(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    let lower = haystack.to_lowercase();
    let mut start = 0;
    while let Some(rel) = lower[start..].find(needle) {
        let idx = start + rel;
        let before_ok = idx == 0 || !is_word_char(lower[..idx].chars().next_back().unwrap_or(' '));
        let after = idx + needle.len();
        let after_ok = after >= lower.len() || !is_word_char(lower[after..].chars().next().unwrap_or(' '));
        if before_ok && after_ok {
            return true;
        }
        start = idx + 1;
    }
    false
}

// ── no-SQL persisted node ─────────────────────────────────

#[derive(Serialize, Deserialize, Clone)]
struct StoredNode {
    id: String,
    content: String,
    memory_type: String,
    total_content_words: usize,
    word_freqs: HashMap<String, i32>,
    model: Option<String>,
    vector: Option<Vec<f32>>,
}

// ── cortex organism (semantic embedding memory, no SQL) ─────

pub struct Cortex {
    pub db_path: String,
    nodes: Arc<Mutex<Vec<StoredNode>>>,
}

impl Cortex {
    pub fn open(db_path: &str) -> Result<Self, std::io::Error> {
        let nodes = if Path::new(db_path).exists() {
            let mut s = String::new();
            fs::File::open(db_path)
                .and_then(|mut f| f.read_to_string(&mut s))
                .ok();
            serde_json::from_str(&s).unwrap_or_default()
        } else {
            Vec::new()
        };
        Ok(Self {
            db_path: db_path.to_string(),
            nodes: Arc::new(Mutex::new(nodes)),
        })
    }

    fn persist(&self) -> Result<(), std::io::Error> {
        if let Some(parent) = Path::new(&self.db_path).parent() {
            fs::create_dir_all(parent)?;
        }
        let nodes = self.nodes.lock();
        let json = serde_json::to_string(&*nodes)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let mut f = fs::File::create(&self.db_path)?;
        f.write_all(json.as_bytes())?;
        Ok(())
    }

    // ── ingest ────────────────────────────────────────

    pub fn ingest(&self, node_id: &str, content: &str, memory_type: &str) -> Result<(), std::io::Error> {
        self.ingest_with_embedding(node_id, content, memory_type, None)
    }

    pub fn ingest_with_embedding(
        &self,
        node_id: &str,
        content: &str,
        memory_type: &str,
        embedding: Option<(&[f32], &str)>,
    ) -> Result<(), std::io::Error> {
        let tokens = tokenize(content);
        let content_tokens = filter_content_words(&tokens);
        if content_tokens.is_empty() {
            return Ok(());
        }

        let mut word_freqs: HashMap<String, i32> = HashMap::new();
        for w in &content_tokens {
            *word_freqs.entry(w.clone()).or_insert(0) += 1;
        }

        let (model, vector) = match embedding {
            Some((vec, m)) => (Some(m.to_string()), Some(vec.to_vec())),
            None => (None, None),
        };

        let node = StoredNode {
            id: node_id.to_string(),
            content: content.to_string(),
            memory_type: memory_type.to_string(),
            total_content_words: content_tokens.len(),
            word_freqs,
            model,
            vector,
        };

        {
            let mut nodes = self.nodes.lock();
            if let Some(existing) = nodes.iter_mut().find(|n| n.id == node_id) {
                *existing = node;
            } else {
                nodes.push(node);
            }
        }
        self.persist()
    }

    // ── vector search ─────────────────────────────────

    pub fn cosine_similarity(
        &self,
        query_vec: &[f32],
        model: &str,
        limit: usize,
    ) -> Result<Vec<SemanticMatch>, std::io::Error> {
        let query_norm = (query_vec.iter().map(|x| x * x).sum::<f32>()).sqrt();
        let nodes = self.nodes.lock();
        let mut scored: Vec<SemanticMatch> = nodes
            .iter()
            .filter(|n| n.model.as_deref() == Some(model))
            .filter_map(|n| {
                let stored = n.vector.as_ref()?;
                if stored.len() != query_vec.len() {
                    return None;
                }
                let dot: f32 = stored.iter().zip(query_vec.iter()).map(|(a, b)| a * b).sum();
                let stored_norm = (stored.iter().map(|x| x * x).sum::<f32>()).sqrt();
                let cos = if query_norm > 0.0 && stored_norm > 0.0 {
                    dot / (query_norm * stored_norm)
                } else {
                    0.0
                };
                Some(SemanticMatch {
                    node_id: n.id.clone(),
                    content: n.content.clone(),
                    memory_type: n.memory_type.clone(),
                    score: cos.max(0.0) as f64,
                    total_content_words: n.total_content_words,
                })
            })
            .collect();

        scored.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(limit);
        Ok(scored)
    }

    // ── recall ────────────────────────────────────────

    pub fn recall_with_protocol(
        &self,
        query: &str,
        limit: usize,
        density: &str,
        _think_in: &[String],
        _output_needed: &[String],
        prepared: &PreparedScoring,
        _model: &str,
    ) -> Result<Vec<MemoryNode>, std::io::Error> {
        let effective_limit = match density {
            "minimal" => 3,
            "balanced" => 5,
            "dense" => 10,
            _ => limit,
        };

        let nodes = self.nodes.lock();
        let mut results: Vec<MemoryNode> = nodes
            .iter()
            .filter_map(|n| {
                let stored = n.vector.as_ref()?;
                if stored.len() != prepared.query_vec.len() {
                    return None;
                }
                let score = Scorer::score(prepared, &n.content, stored);
                let tokens = tokenize(query);
                let content_tokens = filter_content_words(&tokens);
                let matched: Vec<String> = content_tokens
                    .iter()
                    .filter(|t| contains_word(&n.content, t))
                    .cloned()
                    .collect();
                Some(MemoryNode {
                    node_id: n.id.clone(),
                    content: n.content.clone(),
                    memory_type: n.memory_type.clone(),
                    score,
                    matched_words: matched,
                    total_content_words: n.total_content_words,
                })
            })
            .collect();

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(effective_limit);
        Ok(results)
    }

    // ── tag formatting ────────────────────────────────

    pub fn format_tags(&self, query: &str, think_in: &[String], output_needed: &[String]) -> String {
        let query_terms: Vec<String> = query
            .split_whitespace()
            .take(6)
            .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase())
            .collect();

        format!(
            "[persona] query:{} | think_in:{} | output_needed:{}",
            query_terms.join(","),
            think_in.join(","),
            output_needed.join(","),
        )
    }

    // ── maintenance ────────────────────────────────────

    pub fn vacuum(&self) -> Result<(), std::io::Error> {
        self.persist()
    }

    pub fn stats(&self) -> Result<CortexStats, std::io::Error> {
        let nodes = self.nodes.lock();
        let embeddings = nodes.iter().filter(|n| n.vector.is_some()).count();
        Ok(CortexStats {
            nodes: nodes.len() as i64,
            embeddings: embeddings as i64,
        })
    }

    pub fn fast_stats(&self) -> Result<CortexHealth, std::io::Error> {
        self.stats().map(|s| CortexHealth {
            nodes: s.nodes,
            embeddings: s.embeddings,
        })
    }

    pub fn delete_node(&self, node_id: &str) -> Result<(), std::io::Error> {
        {
            let mut nodes = self.nodes.lock();
            nodes.retain(|n| n.id != node_id);
        }
        self.persist()
    }
}

// ── types ──────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize)]
pub struct MemoryNode {
    pub node_id: String,
    pub content: String,
    pub memory_type: String,
    pub score: f64,
    pub matched_words: Vec<String>,
    pub total_content_words: usize,
}

#[derive(Debug, Clone)]
pub struct SemanticMatch {
    pub node_id: String,
    pub content: String,
    pub memory_type: String,
    pub score: f64,
    pub total_content_words: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CortexStats {
    pub nodes: i64,
    pub embeddings: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CortexHealth {
    pub nodes: i64,
    pub embeddings: i64,
}

// ── tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_cortex() -> Cortex {
        let path = format!("/tmp/intueina_test_{}.json", uuid::Uuid::new_v4());
        Cortex::open(&path).unwrap()
    }

    fn cleanup(c: &Cortex) {
        let _ = fs::remove_file(&c.db_path);
    }

    #[test]
    fn test_ingest_creates_nodes() {
        let c = temp_cortex();
        c.ingest("n1", "hello world cortex memory", "note").unwrap();
        let stats = c.stats().unwrap();
        assert_eq!(stats.nodes, 1);
        assert_eq!(stats.embeddings, 0);
        cleanup(&c);
    }

    #[test]
    fn test_ingest_with_embedding_creates_embeddings() {
        let c = temp_cortex();
        c.ingest_with_embedding("n1", "hello world cortex memory", "note", Some((&[0.1f32, 0.2, 0.3], "test-model")))
            .unwrap();
        let stats = c.stats().unwrap();
        assert_eq!(stats.nodes, 1);
        assert_eq!(stats.embeddings, 1);
        cleanup(&c);
    }

    #[test]
    fn test_recall_returns_results() {
        let c = temp_cortex();
        for i in 0..5 {
            c.ingest_with_embedding(
                &format!("r{}", i),
                "deploy pipeline kubernetes ecs docker",
                "note",
                Some((&[0.1f32, 0.2, 0.3], "test-model")),
            )
            .unwrap();
        }
        let results = c.cosine_similarity(&[0.1f32, 0.2, 0.3], "test-model", 5).unwrap();
        assert!(!results.is_empty());
        for r in &results {
            assert!(r.score > 0.0);
        }
        cleanup(&c);
    }

    #[test]
    fn test_delete_node_removes_node() {
        let c = temp_cortex();
        c.ingest("n1", "delete me words", "note").unwrap();
        c.delete_node("n1").unwrap();
        let stats = c.stats().unwrap();
        assert_eq!(stats.nodes, 0);
        cleanup(&c);
    }

    #[test]
    fn test_fast_stats_counts_quickly() {
        let c = temp_cortex();
        for i in 0..5 {
            c.ingest(&format!("fs{}", i), "fast stats test node", "note").unwrap();
        }
        let health = c.fast_stats().unwrap();
        assert_eq!(health.nodes, 5);
        assert_eq!(health.embeddings, 0);
        let full_stats = c.stats().unwrap();
        assert_eq!(health.nodes, full_stats.nodes);
        assert_eq!(health.embeddings, full_stats.embeddings);
        cleanup(&c);
    }

    #[test]
    fn test_recall_with_protocol_empty_on_no_embedding_model() {
        let c = temp_cortex();
        let prepared = PreparedScoring {
            query_vec: vec![0.0_f32; 768],
            term_embeddings: vec![],
        };
        let result = c
            .recall_with_protocol(
                "deploy kubernetes",
                5,
                "balanced",
                &["architecture".into(), "deployment".into()],
                &["pattern-recognition".into()],
                &prepared,
                "Snowflake/snowflake-arctic-embed-m",
            )
            .unwrap();
        assert!(result.is_empty());
        cleanup(&c);
    }

    #[test]
    fn test_format_tags_overrides() {
        let c = temp_cortex();
        let tags = c.format_tags(
            "how do I deploy to prod",
            &["deployment".into(), "security".into()],
            &["design-decision".into()],
        );
        assert!(tags.contains("[persona]"));
        assert!(tags.contains("think_in:deployment,security"));
        assert!(tags.contains("output_needed:design-decision"));
        assert!(tags.contains("how,do,i,deploy,to,prod"));
        cleanup(&c);
    }

    #[test]
    fn test_persist_reopen_retains_nodes() {
        let path = format!("/tmp/intueina_persist_{}.json", uuid::Uuid::new_v4());
        {
            let c = Cortex::open(&path).unwrap();
            c.ingest_with_embedding("n1", "persist me cortex", "note", Some((&[0.1f32, 0.2, 0.3], "m")))
                .unwrap();
        }
        let c2 = Cortex::open(&path).unwrap();
        let stats = c2.stats().unwrap();
        assert_eq!(stats.nodes, 1);
        assert_eq!(stats.embeddings, 1);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_delete_persists_across_reopen() {
        let path = format!("/tmp/intueina_del_{}.json", uuid::Uuid::new_v4());
        {
            let c = Cortex::open(&path).unwrap();
            c.ingest("n1", "delete me cortex", "note").unwrap();
            c.delete_node("n1").unwrap();
        }
        let c2 = Cortex::open(&path).unwrap();
        assert_eq!(c2.stats().unwrap().nodes, 0);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_recall_empty_without_embedding_model() {
        let c = temp_cortex();
        let prepared = PreparedScoring {
            query_vec: vec![0.0_f32; 768],
            term_embeddings: vec![],
        };
        let result = c
            .recall_with_protocol(
                "anything",
                5,
                "balanced",
                &[],
                &[],
                &prepared,
                "Snowflake/snowflake-arctic-embed-m",
            )
            .unwrap();
        assert!(result.is_empty());
        cleanup(&c);
    }
}
