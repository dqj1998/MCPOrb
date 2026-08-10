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

/// Split a namespaced resource URI into its orb slug and the path remainder.
///
/// Format: `orb://{slug}/documents/{id}` → `("my-orb", "/documents/0")`
///
/// Returns `None` for malformed URIs.
///
/// # Examples
///
/// ```
/// use mcporb_gateway_core::router::split_namespaced_resource_uri;
/// assert_eq!(split_namespaced_resource_uri("orb://my-orb/documents/0"), Some(("my-orb", "/documents/0")));
/// assert_eq!(split_namespaced_resource_uri("orb:///documents/0"), None);
/// ```
pub fn split_namespaced_resource_uri(uri: &str) -> Option<(&str, &str)> {
    let rest = uri.strip_prefix(RESOURCE_SCHEME)?;
    let slug_end = rest.find('/')?;
    let slug = &rest[..slug_end];
    if slug.is_empty() {
        return None;
    }
    Some((slug, &rest[slug_end..]))
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
    split_namespaced_resource_uri(uri).map(|(slug, _)| slug)
}

/// Rewrite a namespaced resource URI into the orb-native form the child
/// runtime understands: `orb://{slug}/documents/{id}` → `orb://documents/{id}`.
///
/// Returns `None` for malformed URIs.
///
/// # Examples
///
/// ```
/// use mcporb_gateway_core::router::to_native_resource_uri;
/// assert_eq!(to_native_resource_uri("orb://my-orb/documents/0"), Some("orb://documents/0".to_string()));
/// assert_eq!(to_native_resource_uri("my-orb/documents/0"), None);
/// ```
pub fn to_native_resource_uri(uri: &str) -> Option<String> {
    let (_, remainder) = split_namespaced_resource_uri(uri)?;
    Some(format!("{RESOURCE_SCHEME}{}", remainder.trim_start_matches('/')))
}

/// Prefix an orb-native resource URI (`orb://documents/{id}`) with the slug,
/// producing the namespaced form advertised by `resources/list`.
///
/// Returns `None` for malformed URIs.
///
/// # Examples
///
/// ```
/// use mcporb_gateway_core::router::to_namespaced_resource_uri;
/// assert_eq!(to_namespaced_resource_uri("my-orb", "orb://documents/0"), Some("orb://my-orb/documents/0".to_string()));
/// assert_eq!(to_namespaced_resource_uri("my-orb", "not-an-orb-uri"), None);
/// ```
pub fn to_namespaced_resource_uri(slug: &str, uri: &str) -> Option<String> {
    let rest = uri.strip_prefix(RESOURCE_SCHEME)?;
    if rest.is_empty() {
        return None;
    }
    Some(format!("{RESOURCE_SCHEME}{slug}/{rest}"))
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

    #[test]
    fn split_namespaced_resource_uri_returns_slug_and_remainder() {
        assert_eq!(
            split_namespaced_resource_uri("orb://my-orb/documents/0"),
            Some(("my-orb", "/documents/0"))
        );
    }

    #[test]
    fn split_namespaced_resource_uri_empty_slug() {
        assert_eq!(split_namespaced_resource_uri("orb:///documents/0"), None);
    }

    #[test]
    fn split_namespaced_resource_uri_directory() {
        assert_eq!(
            split_namespaced_resource_uri("orb://my-orb/"),
            Some(("my-orb", "/"))
        );
    }

    #[test]
    fn to_native_resource_uri_strips_slug() {
        assert_eq!(
            to_native_resource_uri("orb://my-orb/documents/0"),
            Some("orb://documents/0".to_string())
        );
    }

    #[test]
    fn to_native_resource_uri_rejects_unprefixed() {
        assert_eq!(to_native_resource_uri("my-orb/documents/0"), None);
    }

    #[test]
    fn to_namespaced_resource_uri_prefixes_slug() {
        assert_eq!(
            to_namespaced_resource_uri("my-orb", "orb://documents/0"),
            Some("orb://my-orb/documents/0".to_string())
        );
    }

    #[test]
    fn to_namespaced_resource_uri_rejects_unprefixed() {
        assert_eq!(to_namespaced_resource_uri("my-orb", "not-an-orb-uri"), None);
    }

    #[test]
    fn resource_uri_rewrite_roundtrip() {
        let namespaced = "orb://test-orb/documents/7";
        let native = to_native_resource_uri(namespaced).unwrap();
        assert_eq!(native, "orb://documents/7");
        let back = to_namespaced_resource_uri("test-orb", &native).unwrap();
        assert_eq!(back, namespaced);
    }
}
