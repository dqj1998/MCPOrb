use mcporb_runtime_core::format::{Chunk, FlatVectorIndex, HnswIndex, RetrievalPlanKind};
use mcporb_runtime_core::search::{SearchMethod, SearchMethodRequest};
use mcporb_runtime_core::{build_bm25_index, DenseRuntime, SearchRequest, SearchRuntime};

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
        make_chunk(
            0,
            "model driven architecture and platform independent model",
        ),
        make_chunk(1, "completely unrelated text about foxes"),
        make_chunk(2, "architecture model transitions and model driven design"),
    ];

    SearchRuntime {
        bm25: build_bm25_index(&chunks),
        #[cfg(feature = "tfidf")]
        tfidf: Some(build_tfidf_index(&chunks)),
        #[cfg(feature = "trigram")]
        trigram: Some(build_trigram_index(&chunks, 1)),
        dense: DenseRuntime::None,
        dense_tier: RetrievalPlanKind::Bm25Only,
    }
}

fn make_vector_store() -> FlatVectorIndex {
    FlatVectorIndex {
        chunk_count: 3,
        dim: 2,
        vectors: vec![1.0, 0.0, 0.0, 1.0, 0.8, 0.2],
        model_id: "test-embed".to_string(),
    }
}

#[test]
fn test_method_request_parse() {
    assert_eq!(
        SearchMethodRequest::from_str("bm25"),
        SearchMethodRequest::Bm25
    );
    assert_eq!(
        SearchMethodRequest::from_str("tfidf"),
        SearchMethodRequest::TfIdf
    );
    assert_eq!(
        SearchMethodRequest::from_str("trigram"),
        SearchMethodRequest::Trigram
    );
    assert_eq!(
        SearchMethodRequest::from_str("hybrid"),
        SearchMethodRequest::Hybrid
    );
    assert_eq!(
        SearchMethodRequest::from_str("unknown"),
        SearchMethodRequest::Auto
    );
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

#[test]
fn test_flat_vector_method_executes_with_query_vector() {
    let lexical = make_runtime();
    let runtime = SearchRuntime {
        bm25: lexical.bm25,
        #[cfg(feature = "tfidf")]
        tfidf: None,
        #[cfg(feature = "trigram")]
        trigram: None,
        dense: DenseRuntime::from_assets(Some(make_vector_store()), None).unwrap(),
        dense_tier: RetrievalPlanKind::Bm25FlatVector,
    };

    let response = runtime
        .search(&SearchRequest {
            query: "ignored lexical query".to_string(),
            top_k: 2,
            method: SearchMethodRequest::FlatVector,
            query_vector: Some(vec![1.0, 0.0]),
            explain: true,
        })
        .unwrap();

    assert_eq!(response.hits[0].chunk_id, 0);
    assert_eq!(response.hits[0].method, SearchMethod::FlatVector);
}

#[test]
fn test_vector_method_requires_query_vector() {
    let lexical = make_runtime();
    let runtime = SearchRuntime {
        bm25: lexical.bm25,
        #[cfg(feature = "tfidf")]
        tfidf: None,
        #[cfg(feature = "trigram")]
        trigram: None,
        dense: DenseRuntime::from_assets(Some(make_vector_store()), None).unwrap(),
        dense_tier: RetrievalPlanKind::Bm25FlatVector,
    };

    let error = runtime
        .search(&SearchRequest {
            query: "ignored lexical query".to_string(),
            top_k: 2,
            method: SearchMethodRequest::FlatVector,
            query_vector: None,
            explain: false,
        })
        .unwrap_err();

    assert!(error.to_string().contains("query_vector"));
}

#[cfg(feature = "hnsw")]
#[test]
fn test_hnsw_method_executes_from_runtime_assets() {
    let lexical = make_runtime();
    let runtime = SearchRuntime {
        bm25: lexical.bm25,
        #[cfg(feature = "tfidf")]
        tfidf: None,
        #[cfg(feature = "trigram")]
        trigram: None,
        dense: DenseRuntime::from_assets(
            Some(make_vector_store()),
            Some(HnswIndex {
                chunk_count: 3,
                dim: 2,
                m: 32,
                ef_construction: 50,
                ef_search: 32,
                model_id: "test-embed".to_string(),
                graph_bytes: Vec::new(),
            }),
        )
        .unwrap(),
        dense_tier: RetrievalPlanKind::Bm25Hnsw,
    };

    let response = runtime
        .search(&SearchRequest {
            query: "ignored lexical query".to_string(),
            top_k: 2,
            method: SearchMethodRequest::FlatVector,
            query_vector: Some(vec![1.0, 0.0]),
            explain: false,
        })
        .unwrap();

    assert!(!response.hits.is_empty());
    assert_eq!(response.hits[0].method, SearchMethod::Hnsw);
}
