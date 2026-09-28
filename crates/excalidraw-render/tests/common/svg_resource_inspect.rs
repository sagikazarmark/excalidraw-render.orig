//! Consumer-facing CSS resource checks, shared by asset and complete-render fuzzing.
pub fn inspect_css(source: &str, allow_bundled_fonts: bool) {
    use cssparser::{Parser, Token};
    fn url_allowed(url: &str, fonts: bool) -> bool {
        url.starts_with('#') || (fonts && url.starts_with("data:font/woff2;base64,"))
    }
    fn tokens(
        parser: &mut Parser<'_>,
        depth: usize,
        fonts: bool,
    ) -> Result<(), cssparser::ParseError<()>> {
        assert!(depth <= 16, "output CSS exceeds bounded nesting");
        while let Ok(token) = parser.next_including_whitespace_and_comments().cloned() {
            match token {
                Token::AtKeyword(name) => assert!(
                    !name.eq_ignore_ascii_case("import")
                        && (fonts || !name.eq_ignore_ascii_case("font-face"))
                ),
                Token::UnquotedUrl(url) => {
                    assert!(url_allowed(&url, fonts), "external CSS URL: {url}")
                }
                Token::Function(name) if name.eq_ignore_ascii_case("url") => {
                    parser
                        .parse_nested_block(|inner| {
                            let url = inner.expect_string()?.clone();
                            assert!(url_allowed(&url, fonts), "external quoted CSS URL: {url}");
                            Ok::<_, cssparser::ParseError<()>>(())
                        })
                        .expect("output URL is well formed");
                }
                Token::Function(_)
                | Token::ParenthesisBlock
                | Token::SquareBracketBlock
                | Token::CurlyBracketBlock => {
                    parser
                        .parse_nested_block(|inner| tokens(inner, depth + 1, fonts))
                        .expect("output CSS block");
                }
                _ => {}
            }
        }
        Ok(())
    }
    tokens(&mut Parser::new(source), 0, allow_bundled_fonts).unwrap();
}
