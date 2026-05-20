use mcporb_runtime_core::format::{Chunk, RetrievalPlanKind};
use mcporb_runtime_core::search::{SearchMethod, SearchMethodRequest};
use mcporb_runtime_core::{build_bm25_index, SearchRequest, SearchRuntime};

#[cfg(feature = "tfidf")]
use mcporb_runtime_core::build_tfidf_index;
#[cfg(feature = "trigram")]
use mcporb_runtime_core::build_trigram_index;

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

fn make_runtime() -> SearchRuntime {
    let chunks = vec![
        make_chunk(0, "model driven architecture and platform independent model"),
        make_chunk(1, "completely unrelated text about foxes"),
        make_chunk(2, "architecture model transitions and model driven design"),
    ];

    SearchRuntime {
        bm25: build_bm25_index(&chunks),
        #[cfg(feature = "tfidf")]
        tfidf: Some(build_tfidf_index(&chunks)),
        #[cfg(feature = "trigram")]
        trigram: Some(build_trigram_index(&chunks, 1)),
        dense_tier: RetrievalPlanKind::Bm25Only,
    }
}

#[test]
fn test_method_request_parse() {
    assert_eq!(SearchMethodRequest::from_str("bm25"), SearchMethodRequest::Bm25);
    assert_eq!(SearchMethodRequest::from_str("tfidf"), SearchMethodRequest::TfIdf);
    assert_eq!(SearchMethodRequest::from_str("trigram"), SearchMethodRequest::Trigram);
    assert_eq!(SearchMethodRequest::from_str("hybrid"), SearchMethodRequest::Hybrid);
    assert_eq!(SearchMethodRequest::from_str("unknown"), SearchMethodRequest::Auto);
}

#[test]
fn test_auto_returns_hybrid_results_when_multiple_rankers_present() {
    let runtime = make_runtime();
    let response = runtime
        .search(&SearchRequest {
            query: "model driven architecture".to_string(),
            top_k: 3,
            method: SearchMethodRequest::Auto,
            query_vector: None,
            explain: true,
        })
        .unwrap();

    assert!(!response.hits.is_empty());
    assert_eq!(response.hits[0].chunk_id, 0);
    assert_eq!(response.hits[0].method, SearchMethod::Hybrid);
    assert!(!response.traces.is_empty());
}

#[test]
fn test_specific_method_executes_without_fusion() {
    let runtime = make_runtime();
    let response = runtime
        .search(&SearchRequest {
            query: "architechture".to_string(),
            top_k: 3,
            method: SearchMethodRequest::Trigram,
            query_vector: None,
            explain: false,
        })
        .unwrap();

    assert!(!response.hits.is_empty());
    assert_eq!(response.hits[0].method, SearchMethod::Trigram);
}