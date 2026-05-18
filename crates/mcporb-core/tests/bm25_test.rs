use mcporb_core::bm25::{build_index, search, tokenize};
use mcporb_core::format::Chunk;

fn make_chunk(id: u32, text: &str) -> Chunk {
    Chunk {
        id,
        document_id: 0,
        section_id: None,
        page: Some(id + 1),
        text: text.to_string(),
        token_count: text.split_whitespace().count(),
    }
}

#[test]
fn test_tokenize() {
    let tokens = tokenize("Hello, World! This is a test.");
    assert!(tokens.contains(&"hello".to_string()));
    assert!(tokens.contains(&"world".to_string()));
    assert!(tokens.contains(&"test".to_string()));
    // Punctuation-only tokens should be filtered
    assert!(!tokens.contains(&",".to_string()));
}

#[test]
fn test_bm25_basic_search() {
    let chunks = vec![
        make_chunk(0, "The quick brown fox jumps over the lazy dog"),
        make_chunk(1, "Model Driven Architecture provides a framework for software development"),
        make_chunk(2, "Platform independent models are central to MDA"),
        make_chunk(3, "The fox was very quick and agile"),
    ];

    let index = build_index(&chunks);
    assert_eq!(index.doc_count, 4);
    assert!(index.vocab.contains_key("fox"));
    assert!(index.vocab.contains_key("model"));

    // Search for "fox" — should return chunks 0 and 3
    let results = search(&index, "fox", 5);
    assert!(!results.is_empty());
    let ids: Vec<u32> = results.iter().map(|(id, _)| *id).collect();
    assert!(ids.contains(&0) || ids.contains(&3));

    // Search for "model driven architecture" — should return chunk 1
    let results = search(&index, "model driven architecture", 3);
    assert!(!results.is_empty());
    assert_eq!(results[0].0, 1, "chunk 1 should rank first for MDA query");
}

#[test]
fn test_bm25_empty_query() {
    let chunks = vec![make_chunk(0, "some text here")];
    let index = build_index(&chunks);
    let results = search(&index, "", 5);
    assert!(results.is_empty());
}

#[test]
fn test_bm25_unknown_term() {
    let chunks = vec![make_chunk(0, "hello world")];
    let index = build_index(&chunks);
    let results = search(&index, "zzzyyyxxx", 5);
    assert!(results.is_empty());
}

#[test]
fn test_postcard_roundtrip() {
    let chunks = vec![
        make_chunk(0, "first document about software architecture"),
        make_chunk(1, "second document about model driven development"),
    ];
    let index = build_index(&chunks);
    let bytes = postcard::to_allocvec(&index).unwrap();
    let restored: mcporb_core::format::Bm25Index = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(restored.doc_count, index.doc_count);
    assert_eq!(restored.vocab.len(), index.vocab.len());
    // Search on restored index should give same results
    let r1 = search(&index, "software architecture", 3);
    let r2 = search(&restored, "software architecture", 3);
    assert_eq!(r1, r2);
}
