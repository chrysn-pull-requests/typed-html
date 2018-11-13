use pom::combinator::*;
use pom::{Error, Parser};
use proc_macro::{
    Delimiter, Diagnostic, Group, Ident, Level, Literal, Punct, TokenStream, TokenTree,
};

pub fn unit<'a, I: 'a, A: Clone>(value: A) -> Combinator<impl Parser<'a, I, Output = A>> {
    comb(move |_, start| Ok((value.clone(), start)))
}

pub fn punct<'a>(punct: char) -> Combinator<impl Parser<'a, TokenTree, Output = Punct>> {
    comb(move |input: &[TokenTree], start| match input.get(start) {
        Some(TokenTree::Punct(p)) if p.as_char() == punct => Ok((p.clone(), start + 1)),
        _ => Err(Error::Mismatch {
            message: format!("expected {:?}", punct),
            position: start,
        }),
    })
}

pub fn ident<'a>() -> Combinator<impl Parser<'a, TokenTree, Output = Ident>> {
    comb(|input: &[TokenTree], start| match input.get(start) {
        Some(TokenTree::Ident(i)) => Ok((i.clone(), start + 1)),
        _ => Err(Error::Mismatch {
            message: "expected identifier".to_string(),
            position: start,
        }),
    })
}

pub fn ident_match<'a>(name: String) -> Combinator<impl Parser<'a, TokenTree, Output = ()>> {
    comb(move |input: &[TokenTree], start| match input.get(start) {
        Some(TokenTree::Ident(i)) => {
            if i.to_string() == name {
                Ok(((), start + 1))
            } else {
                Err(Error::Mismatch {
                    message: format!("expected '</{}>', found '</{}>'", name, i.to_string()),
                    position: start,
                })
            }
        }
        _ => Err(Error::Mismatch {
            message: "expected identifier".to_string(),
            position: start,
        }),
    })
}

pub fn literal<'a>() -> Combinator<impl Parser<'a, TokenTree, Output = Literal>> {
    comb(|input: &[TokenTree], start| match input.get(start) {
        Some(TokenTree::Literal(l)) => Ok((l.clone(), start + 1)),
        _ => Err(Error::Mismatch {
            message: "expected literal".to_string(),
            position: start,
        }),
    })
}

pub fn group<'a>() -> Combinator<impl Parser<'a, TokenTree, Output = Group>> {
    comb(|input: &[TokenTree], start| match input.get(start) {
        Some(TokenTree::Group(g)) => Ok((g.clone(), start + 1)),
        _ => Err(Error::Mismatch {
            message: "expected group".to_string(),
            position: start,
        }),
    })
}

fn to_stream<'a, I: IntoIterator<Item = &'a TokenTree>>(tokens: I) -> TokenStream {
    let mut stream = TokenStream::new();
    stream.extend(tokens.into_iter().cloned());
    stream
}

pub fn type_spec<'a>() -> Combinator<impl Parser<'a, TokenTree, Output = TokenStream>> {
    let valid = ident().map(TokenTree::Ident)
        | punct(':').map(TokenTree::Punct)
        | punct('<').map(TokenTree::Punct)
        | punct('>').map(TokenTree::Punct)
        | punct('&').map(TokenTree::Punct)
        | punct('\'').map(TokenTree::Punct);
    valid.repeat(1..).collect().map(to_stream)
}

pub fn dotted_ident<'a>() -> Combinator<impl Parser<'a, TokenTree, Output = TokenTree>> {
    (ident()
        + ((punct('.') + ident()).discard() | (punct(':').repeat(2) + ident()).discard())
            .repeat(0..))
    .collect()
    .map(|tokens| {
        if tokens.len() == 1 {
            tokens[0].clone()
        } else {
            Group::new(Delimiter::Brace, to_stream(tokens)).into()
        }
    })
}

/// Read a sequence of idents and dashes, and merge them into a single ident
/// with the dashes replaced by underscores.
pub fn html_ident<'a>() -> Combinator<impl Parser<'a, TokenTree, Output = Ident>> {
    let start = ident();
    let next = punct('-') * ident();
    (start * next.repeat(0..)).collect().map(|stream| {
        let (span, name) = stream
            .iter()
            .fold((None, String::new()), |(span, name), token| {
                (
                    match span {
                        None => Some(token.span()),
                        Some(span) => span.join(token.span()),
                    },
                    match token {
                        TokenTree::Ident(ident) => name + &ident.to_string(),
                        TokenTree::Punct(_) => name + "_",
                        _ => unreachable!(),
                    },
                )
            });
        Ident::new(&name, span.unwrap())
    })
}

/// Turn a parser error into a proc_macro diagnostic.
pub fn parse_error(input: &[TokenTree], error: &pom::Error) -> Diagnostic {
    match error {
        pom::Error::Incomplete => Diagnostic::new(Level::Error, "unexpected end of macro!"),
        pom::Error::Mismatch { message, position } => {
            Diagnostic::spanned(input[*position].span(), Level::Error, message.as_str())
        }
        pom::Error::Conversion { message, position } => {
            Diagnostic::spanned(input[*position].span(), Level::Error, message.as_str())
        }
        pom::Error::Expect {
            message,
            position,
            inner,
        } => {
            let mut diag =
                Diagnostic::spanned(input[*position].span(), Level::Error, message.as_str());
            let child = parse_error(input, &inner);
            diag.span_error(child.spans(), child.message())
        }
        pom::Error::Custom {
            message,
            position,
            inner,
        } => {
            let mut diag =
                Diagnostic::spanned(input[*position].span(), Level::Error, message.as_str());
            if let Some(inner) = inner {
                let child = parse_error(input, &inner);
                diag.span_error(child.spans(), child.message())
            } else {
                diag
            }
        }
    }
}
