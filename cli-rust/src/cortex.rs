use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;

// SPIM guts — positional, lossless, passive
use positional_memory::compact::CompactPositional;
use positional_memory::store;

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

fn is_stopword(w: &str) -> bool { STOPWORDS.contains(&w) }
fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase().split_whitespace().map(|w| w.trim_matches(|c: char| !c.is_alphanumeric())).filter(|w| w.len() >= 2).map(|s| s.to_string()).collect()
}
fn filter_content_words(words: &[String]) -> Vec<String> { words.iter().filter(|w| !is_stopword(w)).cloned().collect() }
fn is_word_char(c: char) -> bool { c.is_alphanumeric() }
fn contains_word(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() { return false; }
    let lower = haystack.to_lowercase();
    let mut start = 0;
    while let Some(rel) = lower[start..].find(needle) {
        let idx = start + rel;
        let before_ok = idx == 0 || !is_word_char(lower[..idx].chars().next_back().unwrap_or(' '));
        let after = idx + needle.len();
        let after_ok = after >= lower.len() || !is_word_char(lower[after..].chars().next().unwrap_or(' '));
        if before_ok && after_ok { return true; }
        start = idx + 1;
    }
    false
}

// Legacy JSON shape for migration
#[derive(Serialize, Deserialize, Clone)]
struct StoredNodeLegacy {
    id: String, content: String, memory_type: String,
    #[serde(default)] total_content_words: usize,
    #[serde(default)] word_freqs: HashMap<String, i32>,
    model: Option<String>, vector: Option<Vec<f32>>,
}

// SPIM-backed cortex: inherent read/write/update by design
pub struct Cortex {
    pub db_path: String,
    inner: Arc<Mutex<CompactPositional>>,
    // id -> (memory_type, spim_id)
    meta: Arc<Mutex<HashMap<String, (String, u32)>>>,
}

impl Cortex {
    pub fn open(db_path: &str) -> Result<Self, std::io::Error> {
        let path = Path::new(db_path);
        let mut inner = if path.exists() {
            // Try SPIM format first
            match store::load(path) {
                Ok(c) => {
                    eprintln!("[intueina] loaded SPIM with {} memories", c.num_memories);
                    c
                },
                Err(e) => {
                    eprintln!("[intueina] SPIM load failed ({}), trying legacy JSON migration", e);
                    // Try legacy JSON migration
                    let mut s = String::new();
                    let read_res = fs::File::open(path).and_then(|mut f| f.read_to_string(&mut s));
                    eprintln!("[intueina] read legacy JSON: {:?} len {}", read_res.is_ok(), s.len());
                    match serde_json::from_str::<Vec<StoredNodeLegacy>>(&s) {
                        Ok(legacy) => {
                            eprintln!("[intueina] migrating {} legacy nodes to SPIM", legacy.len());
                            let mut c = CompactPositional::new();
                            for n in &legacy {
                                c.insert_text(&n.content);
                            }
                            match store::save(&c, path) {
                                Ok(_) => eprintln!("[intueina] migrated and saved SPIM with {} memories", c.num_memories),
                                Err(e) => eprintln!("[intueina] save failed: {}", e),
                            }
                            c
                        },
                        Err(e) => {
                            eprintln!("[intueina] legacy parse failed: {}", e);
                            CompactPositional::new()
                        }
                    }
                }
            }
        } else { CompactPositional::new() };
        // Rebuild meta from SPIM memories if possible (id mapping is by content hash, we keep empty for now)
        Ok(Self { db_path: db_path.to_string(), inner: Arc::new(Mutex::new(inner)), meta: Arc::new(Mutex::new(HashMap::new())) })
    }

    fn persist(&self) -> Result<(), std::io::Error> {
        let inner = self.inner.lock();
        store::save(&*inner, &self.db_path)
    }

    // ——— inherent WRITE by design ———
    // agent_driven, passive, append_only: every ingest is an incident, never mutates
    pub fn ingest(&self, node_id: &str, content: &str, memory_type: &str) -> Result<(), std::io::Error> {
        self.ingest_with_embedding(node_id, content, memory_type, None)
    }
    pub fn ingest_with_embedding(&self, node_id: &str, content: &str, memory_type: &str, _embedding: Option<(&[f32], &str)>) -> Result<(), std::io::Error> {
        let tokens = tokenize(content);
        if filter_content_words(&tokens).is_empty() { return Ok(()); }
        let mut inner = self.inner.lock();
        let spim_id = inner.insert_text(content);
        {
            let mut meta = self.meta.lock();
            meta.insert(node_id.to_string(), (memory_type.to_string(), spim_id));
        }
        // 5-tag funnel — one word one time, memoized universe, tags of tags can be tagged
        // For each distinct content word, create a separate tag memory if not already memoized
        let distinct: std::collections::HashSet<String> = filter_content_words(&tokens).into_iter().collect();
        for w in distinct {
            // Check if word already has a tag memory (by trying to recall it) — cheap check via and_query
            let already = {
                let toks = vec![w.as_str()];
                !inner.and_query(&toks).is_empty() || {
                    // Also check if a tag memory for this word already exists by looking for "word len:" pattern
                    let hits = inner.match_c3(&w).0;
                    hits.iter().any(|&mid| inner.reconstruct_string(mid).map(|s| s.contains(&format!("{} len", w))).unwrap_or(false))
                }
            };
            if already {
                // Check more precisely: if any existing memory is exactly the tag memory for this word, skip
                // For now, skip if we already have a tag memory for this word by checking if any memory reconstructs to start with "w len"
                let has_tag = {
                    let hits = inner.match_c3(&format!("{} len", w)).0;
                    !hits.is_empty()
                };
                if has_tag { continue; }
            }
            // Generate 5 tags via SPIM's processing layer — split what chain for LIKE
            let tags = positional_memory::tag::tag(&w);
            let what_parts = tags.what.strip_prefix("what:").unwrap_or(&tags.what).replace('_', " ");
            let tag_content = format!("{} {} {} {} {} {}", w, tags.len, tags.topo, what_parts, tags.ent, tags.res);
            inner.insert_text(&tag_content);
        }
        drop(inner);
        self.persist()
    }

    // ——— inherent READ by design ———
    // positional chain p→p+1 + rare-first + reconstruct from positions alone
    pub fn cosine_similarity(&self, _query_vec: &[f32], _model: &str, limit: usize) -> Result<Vec<SemanticMatch>, std::io::Error> {
        // Gutted: cosine is dead — vector is not stored. Return empty to force positional recall.
        let _ = limit;
        Ok(vec![])
    }

    pub fn recall_with_protocol(
        &self,
        query: &str,
        limit: usize,
        density: &str,
        _think_in: &[String],
        _output_needed: &[String],
        _prepared: &as_not_as_protocol::scorer::PreparedScoring,
        _model: &str,
    ) -> Result<Vec<MemoryNode>, std::io::Error> {
        let effective_limit = match density { "minimal"=>3, "balanced"=>5, "dense"=>10, _=>limit };
        let inner = self.inner.lock();
        // Inherent lexical understanding: expand query via 5-tag funnel at read time
        // If query is single word, expand via its what/telic to find shared whats
        let query_words: Vec<String> = tokenize(query);
        let mut expanded_hits: Vec<u32> = Vec::new();
        if query_words.len()==1 {
            let w = &query_words[0];
            let tags = positional_memory::tag::tag(w);
            // Find tag memories that share what prefix (macro) — e.g., puppy animal finds dog/cat via animal
            let what_prefix = tags.what.split(':').nth(1).and_then(|s| s.split('_').next()).unwrap_or("");
            if !what_prefix.is_empty() && what_prefix!="unknown" {
                // Use and_query to find tag memories containing this what component (now stored as separate token "animal")
                let tag_hits = inner.and_query(&[Box::leak(what_prefix.to_string().into_boxed_str()) as &str]);
                // Collect original words from those tag memories
                let mut related_words: Vec<String> = Vec::new();
                for mid in tag_hits {
                    if let Some(content) = inner.reconstruct_string(mid) {
                        if let Some(word) = content.split_whitespace().next() {
                            if word != w { related_words.push(word.to_string()); }
                        }
                    }
                }
                // Now find original memories containing those related words plus the query word
                for rw in related_words {
                    let hits = inner.and_query(&[rw.as_str()]);
                    expanded_hits.extend(hits);
                }
            }
        }
        // Positional recall: try exact phrase first, then AND, then BM25, plus expanded hits
        let mut hits: Vec<u32> = inner.match_c3(query).0;
        if hits.is_empty() {
            let toks_ref: Vec<&str> = query_words.iter().map(|s| s.as_str()).collect();
            hits = inner.and_query(&toks_ref);
        }
        // Merge expanded lexical hits
        hits.extend(expanded_hits);
        hits.sort_unstable();
        hits.dedup();
        if hits.is_empty() {
            let ranked = inner.bm25(query, 1.5, 0.75);
            hits = ranked.iter().take(effective_limit).map(|(m,_)| *m).collect();
        }
        hits.sort_unstable();
        hits.truncate(effective_limit);
        let mut out = Vec::new();
        for mid in &hits {
            if let Some(content) = inner.reconstruct_string(*mid) {
                if content.contains("len:") || content.contains("what:") { continue; }
                let tokens = tokenize(query);
                let matched: Vec<String> = filter_content_words(&tokens).into_iter().filter(|t| contains_word(&content, t)).collect();
                let total = filter_content_words(&tokenize(&content)).len();
                out.push(MemoryNode{ node_id: mid.to_string(), content, memory_type: "note".into(), score: 1.0, matched_words: matched, total_content_words: total });
            }
        }
        if out.is_empty() && !hits.is_empty() {
            for mid in hits.iter().take(effective_limit) {
                if let Some(content) = inner.reconstruct_string(*mid) {
                    out.push(MemoryNode{ node_id: mid.to_string(), content: content.clone(), memory_type: "note".into(), score: 0.5, matched_words: vec![], total_content_words: filter_content_words(&tokenize(&content)).len() });
                    if out.len()>=effective_limit { break; }
                }
            }
        }
        Ok(out)
    }

    pub fn format_tags(&self, query: &str, think_in: &[String], output_needed: &[String]) -> String {
        let query_terms: Vec<String> = query.split_whitespace().take(6).map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase()).collect();
        format!("[persona] query:{} | think_in:{} | output_needed:{}", query_terms.join(","), think_in.join(","), output_needed.join(","))
    }

    // ——— inherent UPDATE by design ———
    // append_only: never UPDATE or DELETE. Update = new incident that supersedes. Delete = tombstone.
    pub fn vacuum(&self) -> Result<(), std::io::Error> { self.persist() }
    pub fn stats(&self) -> Result<CortexStats, std::io::Error> {
        let inner = self.inner.lock();
        Ok(CortexStats{ nodes: inner.num_memories as i64, embeddings: 0 })
    }
    pub fn fast_stats(&self) -> Result<CortexHealth, std::io::Error> {
        self.stats().map(|s| CortexHealth{ nodes: s.nodes, embeddings: s.embeddings })
    }
    pub fn delete_node(&self, node_id: &str) -> Result<(), std::io::Error> {
        // Inherent update: tombstone append, not delete. We still remove meta mapping but keep SPIM history.
        {
            let mut meta = self.meta.lock();
            meta.remove(node_id);
        }
        // Append a tombstone memory so history proves deletion without erasing
        let mut inner = self.inner.lock();
        inner.insert_text(&format!("tombstone {}", node_id));
        drop(inner);
        self.persist()
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct MemoryNode { pub node_id: String, pub content: String, pub memory_type: String, pub score: f64, pub matched_words: Vec<String>, pub total_content_words: usize, }
#[derive(Debug, Clone)]
pub struct SemanticMatch { pub node_id: String, pub content: String, pub memory_type: String, pub score: f64, pub total_content_words: usize, }
#[derive(Debug, Clone, serde::Serialize)]
pub struct CortexStats { pub nodes: i64, pub embeddings: i64, }
#[derive(Debug, Clone, serde::Serialize)]
pub struct CortexHealth { pub nodes: i64, pub embeddings: i64, }

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    #[test]
    fn test_ingest_creates_nodes() {
        let path = format!("/tmp/intueina_test_{}.json", uuid::Uuid::new_v4());
        let c = Cortex::open(&path).unwrap();
        c.ingest("n1", "hello world cortex memory", "note").unwrap();
        // 5-tag funnel: 1 original + tag memories for each distinct word (hello, world, cortex, memory) = up to 5
        assert!(c.stats().unwrap().nodes >= 1);
        assert_eq!(c.stats().unwrap().embeddings, 0);
        let _ = fs::remove_file(&path);
    }
    #[test]
    fn test_recall_returns_results() {
        let path = format!("/tmp/intueina_test_{}.json", uuid::Uuid::new_v4());
        let c = Cortex::open(&path).unwrap();
        for i in 0..5 { c.ingest(&format!("r{}",i), "deploy pipeline kubernetes ecs docker", "note").unwrap(); }
        let prepared = as_not_as_protocol::scorer::PreparedScoring{ query_vec: vec![0.0; 1], term_embeddings: vec![] };
        let res = c.recall_with_protocol("deploy pipeline", 5, "balanced", &[], &[], &prepared, "x").unwrap();
        assert!(!res.is_empty());
        let _ = fs::remove_file(&path);
    }
}
