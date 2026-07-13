//! Namespace parsing for multi-Orb routing.
//!
//! Tool names use `{slug}__{method}` format:
//!   `my-orb__search_knowledge` → slug=`my-orb`, method=`search_knowledge`
//!
//! Resource URIs use `orb://{slug}/documents/{id}` format:
//!   `orb://my-orb/documents/0` → slug=`my-orb`, doc_id=`0`

/// Separator between orb slug and method name in namespaced tool names.
pub const NAMESPACE_SEP: &str = "__";

/// Scheme prefix for resource URIs.
const RESOURCE_SCHEME: &str = "orb://";

/// Parse a namespaced tool name into `(slug, method)`.
///
/// Returns `None` if the name does not contain a namespace separator.
///
/// # Examples
///
/// ```
/// use mcporb_gateway_core::router::parse_tool_name;
/// assert_eq!(parse_tool_name("my-orb__search_knowledge"), Some(("my-orb", "search_knowledge")));
/// assert_eq!(parse_tool_name("search_knowledge"), None);
/// ```
pub fn parse_tool_name(name: &str) -> Option<(&str, &str)> {
    let (slug, method) = name.split_once(NAMESPACE_SEP)?;
    if slug.is_empty() || method.is_empty() {
        return None;
    }
    Some((slug, method))
}

/// Extract the orb slug from a namespaced resource URI.
///
/// Format: `orb://{slug}/documents/{id}`
///
/// Returns `None` for malformed URIs.
///
/// # Examples
///
/// ```
/// use mcporb_gateway_core::router::extract_slug_from_resource_uri;
/// assert_eq!(extract_slug_from_resource_uri("orb://my-orb/documents/0"), Some("my-orb"));
/// assert_eq!(extract_slug_from_resource_uri("orb:///documents/0"), None);
/// ```
pub fn extract_slug_from_resource_uri(uri: &str) -> Option<&str> {
    let rest = uri.strip_prefix(RESOURCE_SCHEME)?;
    let slug_end = rest.find('/')?;
    let slug = &rest[..slug_end];
    if slug.is_empty() {
        return None;
    }
    Some(slug)
}

/// Build a namespaced tool name from slug and method.
///
/// # Examples
///
/// ```
/// use mcporb_gateway_core::router::build_namespaced_tool_name;
/// assert_eq!(build_namespaced_tool_name("my-orb", "search_knowledge"), "my-orb__search_knowledge");
/// ```
pub fn build_namespaced_tool_name(slug: &str, method: &str) -> String {
    format!("{slug}{NAMESPACE_SEP}{method}")
}

/// Build a namespaced resource URI from slug and document id.
///
/// # Examples
///
/// ```
/// use mcporb_gateway_core::router::build_namespaced_resource_uri;
/// assert_eq!(build_namespaced_resource_uri("my-orb", 0), "orb://my-orb/documents/0");
/// ```
pub fn build_namespaced_resource_uri(slug: &str, doc_id: u32) -> String {
    format!("{RESOURCE_SCHEME}{slug}/documents/{doc_id}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_tool_name_normal() {
        assert_eq!(
            parse_tool_name("my-orb__search_knowledge"),
            Some(("my-orb", "search_knowledge"))
        );
    }

    #[test]
    fn parse_tool_name_no_separator() {
        assert_eq!(parse_tool_name("search_knowledge"), None);
    }

    #[test]
    fn parse_tool_name_empty_slug() {
        assert_eq!(parse_tool_name("__search_knowledge"), None);
    }

    #[test]
    fn parse_tool_name_empty_method() {
        assert_eq!(parse_tool_name("my-orb__"), None);
    }

    #[test]
    fn parse_tool_name_multiple_separators() {
        // split_once only splits on first occurrence
        assert_eq!(
            parse_tool_name("my__orb__search"),
            Some(("my", "orb__search"))
        );
    }

    #[test]
    fn parse_tool_name_hyphenated_slug() {
        assert_eq!(
            parse_tool_name("my-knowledge-orb__search_knowledge"),
            Some(("my-knowledge-orb", "search_knowledge"))
        );
    }

    #[test]
    fn extract_slug_from_resource_uri_normal() {
        assert_eq!(
            extract_slug_from_resource_uri("orb://my-orb/documents/0"),
            Some("my-orb")
        );
    }

    #[test]
    fn extract_slug_from_resource_uri_empty_slug() {
        assert_eq!(extract_slug_from_resource_uri("orb:///documents/0"), None);
    }

    #[test]
    fn extract_slug_from_resource_uri_no_scheme() {
        assert_eq!(extract_slug_from_resource_uri("/documents/0"), None);
    }

    #[test]
    fn extract_slug_from_resource_uri_trailing_path() {
        assert_eq!(
            extract_slug_from_resource_uri("orb://another-orb/documents/42"),
            Some("another-orb")
        );
    }

    #[test]
    fn build_namespaced_tool_name_roundtrip() {
        let slug = "my-orb";
        let method = "search_knowledge";
        let built = build_namespaced_tool_name(slug, method);
        let parsed = parse_tool_name(&built);
        assert_eq!(parsed, Some((slug, method)));
    }

    #[test]
    fn build_namespaced_resource_uri_format() {
        let uri = build_namespaced_resource_uri("test-orb", 7);
        assert_eq!(uri, "orb://test-orb/documents/7");
        assert_eq!(extract_slug_from_resource_uri(&uri), Some("test-orb"));
    }
}
