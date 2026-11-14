//! The `/scalar` API reference, from the weebo-api-contract template.
//! No bundle is shipped yet (plan D10): the page embeds the spec, loads
//! nothing from a third party, and says what is missing.

use utoipa_scalar::{Scalar, Servable as _};

/// Same-origin path the bundle will be served from once it ships.
pub const SCALAR_BUNDLE_PATH: &str = "assets/scalar/standalone.js";

/// `$spec` is substituted by `Scalar::to_html` inside a JSON block, so the
/// CSP needs no `'unsafe-inline'` for scripts.
fn scalar_unavailable_html() -> String {
    format!(
        r#"<!doctype html>
<html>
<head>
    <title>kura API</title>
    <meta charset="utf-8"/>
    <meta name="viewport" content="width=device-width, initial-scale=1"/>
</head>
<body>
<h1>API reference unavailable</h1>
<p>This build serves no Scalar bundle (<code>{SCALAR_BUNDLE_PATH}</code>). The OpenAPI document is embedded below.</p>
<script id="api-reference" type="application/json">
    $spec
</script>
</body>
</html>
"#
    )
}

pub fn scalar(openapi: utoipa::openapi::OpenApi) -> Scalar<utoipa::openapi::OpenApi> {
    Scalar::with_url("/scalar", openapi).custom_html(scalar_unavailable_html())
}

/// Sent only on `/scalar`. `connect-src 'self'` stops the bundle's hardcoded
/// third-party origin once one ships.
pub const API_DOCS_CSP: &str = "default-src 'none'; \
     script-src 'self'; \
     style-src 'self' 'unsafe-inline'; \
     img-src 'self' data: blob:; \
     font-src 'self' data:; \
     connect-src 'self'; \
     worker-src 'self' blob:; \
     base-uri 'none'; \
     form-action 'none'; \
     frame-ancestors 'none'";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_reference_loads_nothing_from_a_third_party() {
        let html = scalar_unavailable_html();
        assert!(!html.contains("//"), "{html}");
        assert!(!html.contains("scalar.com"));
        assert!(html.contains("$spec"));
        let rendered = scalar(crate::app::openapi_spec()).to_html();
        assert!(rendered.contains("\"openapi\""), "the spec is embedded");
        assert!(!rendered.contains("<script src="));
    }
}
