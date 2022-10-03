use chumsky::{prelude::*, text::Character};
use std::ops::Range;

#[derive(Clone, Debug)]
enum Delim {
    Block,
    Parenthesis,
    Curly,
    Interpolation,
}

#[derive(Clone, Debug)]
enum IntegerKind {
    I8,
    I16,
    I32,
    I64,
    Isz,
    U8,
    U16,
    U32,
    U64,
    Usz,
}

#[derive(Clone, Debug)]
enum Operator {
    Add,
    Equals,
    MemberAccess,
}

#[derive(Clone, Debug)]
enum Token {
    Ident(String),
    StringLiteral(String),
    Int(u64, Option<IntegerKind>),
    Operator(Operator),
    Comment,
    Open(Delim),
    Close(Delim),
}

#[derive(Clone, Debug)]
enum TokenTree {
    Token(Token),
    Tree(Delim, Vec<Spanned<TokenTree>>),
}

type Span = Range<usize>;

type Spanned<T> = (T, Span);

fn lexer() -> impl Parser<char, Vec<Spanned<TokenTree>>, Error = Simple<char>> {
    let tt = recursive(|tt| {
        let operator = just('+')
            .to(Operator::Add)
            .or(just('=').to(Operator::Equals))
            .or(just('.').to(Operator::MemberAccess))
            .map(Token::Operator)
            .labelled("Operator");

        let line_ws = filter(|c: &char| c.is_inline_whitespace()).repeated();

        let ident = text::ident().map(Token::Ident).labelled("Identifier");
        let int = text::int(10)
            .from_str()
            .unwrapped()
            .then(
                text::keyword("i8")
                    .to(IntegerKind::I8)
                    .or(text::keyword("i16").to(IntegerKind::I16))
                    .or(text::keyword("i32").to(IntegerKind::I32))
                    .or(text::keyword("i64").to(IntegerKind::I64))
                    .or(text::keyword("isz").to(IntegerKind::Isz))
                    .or(text::keyword("u8").to(IntegerKind::U8))
                    .or(text::keyword("u16").to(IntegerKind::U16))
                    .or(text::keyword("u32").to(IntegerKind::U32))
                    .or(text::keyword("u64").to(IntegerKind::U64))
                    .or(text::keyword("usz").to(IntegerKind::Usz))
                    .or_not(),
            )
            .map(|(i, t)| Token::Int(i, t))
            .labelled("Integer");

        let escape = just('\\').ignore_then(
            just('\\')
                .or(just('$'))
                .or(just('"'))
                .or(just('n').to('\n'))
                .or(just('r').to('\r'))
                .or(just('t').to('\t'))
                .or(just('0').to('\0')),
        );

        let literal = filter(|c| *c != '"' && *c != '\\' && *c != '$')
            .or(escape)
            .or(just('$').then_ignore(just('{').not().rewind()))
            .repeated()
            .at_least(1)
            .collect()
            .map(Token::StringLiteral)
            .map_with_span(|s, span| (TokenTree::Token(s), span))
            .labelled("String literal");

        let interpolation = tt
            .clone()
            .padded()
            .repeated()
            .delimited_by(just("${"), just('}'))
            .map_with_span(|tts, span| (TokenTree::Tree(Delim::Curly, tts), span));

        let string = interpolation
            .or(literal)
            .repeated()
            .delimited_by(just('"'), just('"'))
            .map(|tts| TokenTree::Tree(Delim::Interpolation, tts))
            .labelled("String");

        let single = int.or(ident);

        // Token with extra tokens separated by non 0 length inline whitespace ahead
        let sequential = single
            .clone()
            .then_ignore(line_ws.at_least(1).then(single.clone()).rewind())
            .labelled("Non-final token in a sequence");

        // Token with no other tokens ahead
        let last = single
            .clone()
            .then_ignore(line_ws.then(single).not().rewind())
            .labelled("Final token in a sequence");

        let single = sequential.or(last).or(operator).map(TokenTree::Token);

        let token_tree = tt
            .padded()
            .repeated()
            .delimited_by(just('('), just(')'))
            .map(|tts| TokenTree::Tree(Delim::Parenthesis, tts));

        let single_line = just("//")
            .then(take_until(text::newline().rewind()))
            .ignored()
            .to(Token::Comment);
        let multi_line = just("/*")
            .then(take_until(just("*/")))
            .ignored()
            .to(Token::Comment);
        // The comments will get filtered out in the next stage,
        // but parsing them here to preserve semantic indentation
        let comment = single_line.or(multi_line).map(TokenTree::Token);

        single
            .or(string)
            .or(token_tree)
            .or(comment)
            .map_with_span(|tt, span| (tt, span))
    });

    text::semantic_indentation(tt, |tts, span| (TokenTree::Tree(Delim::Block, tts), span))
        .then_ignore(end())
}

fn main() {
    let src = include_str!("hello.an");
    match lexer().parse(src) {
        Ok(tts) => println!("{:#?}", tts),
        Err(err) => println!("Parse error: {:#?}", err),
    }
}
