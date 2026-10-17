use proc_macro2::{Delimiter, TokenStream, TokenTree};
use quote::TokenStreamExt;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{Ident, Token, braced, bracketed, parenthesized};

use crate::*;

impl Parse for DottedName {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let inner = Punctuated::parse_separated_nonempty(input)?;
        Ok(Self(inner))
    }
}

impl Parse for PathSepName {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let prefix = if input.peek(Token![crate]) {
            let _ = input.parse::<Token![crate]>()?;
            let _ = input.parse::<Token![::]>()?;
            PathSepPrefix::Crate
        } else if input.peek(Token![self]) {
            let _ = input.parse::<Token![self]>()?;
            let _ = input.parse::<Token![::]>()?;
            PathSepPrefix::This
        } else if input.peek(Token![super]) {
            let _ = input.parse::<Token![super]>()?;
            let _ = input.parse::<Token![::]>()?;
            PathSepPrefix::Super
        } else {
            PathSepPrefix::None
        };

        let mut parts = Vec::new();

        while input.peek2(Token![::]) {
            let part = input.parse()?;
            let _ = input.parse::<Token![::]>()?;
            parts.push(part);
        }

        Ok(Self {
            prefix,
            parts,
        })
    }
}

impl Parse for JavaClassDef {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let _ = input.parse::<kw::package>()?;
        let package: DottedName = input.parse()?;
        let _ = input.parse::<Token![;]>()?;

        let mut imports = Vec::new();
        let mut uses = Vec::new();

        let mut errors = Vec::new();

        while let is_import = input.peek(kw::import) && (is_import || input.peek(Token![use])) {
            if let Err(e) = (|| -> syn::Result<()> {
                if is_import {
                    let _ = input.parse::<kw::import>()?;
                    let item = input.parse()?;
                    let _ = input.parse::<Token![;]>()?;

                    imports.push(JavaImport {
                        item,
                    });
                } else {
                    let _ = input.parse::<Token![use]>()?;
                    let mut rust_path: PathSepName = input.parse()?;
                    let java_path: DottedName = input.parse()?;
                    let _ = input.parse::<Token![;]>()?;

                    rust_path.parts.push(java_path.0.last().unwrap().clone());

                    uses.push(JavaUse {
                        rust_path,
                        java_path,
                    })
                }
                Ok(())
            })() {
                errors.push(e.to_compile_error());
                input.step(|cursor| {
                    let mut rest = *cursor;
                    while let Some((tt, next)) = rest.token_tree() {
                        match tt {
                            TokenTree::Punct(p) if p.as_char() == ';' => {
                                return Ok(((), next))
                            },
                            _ => rest = next,
                        }
                    }
                    Ok(((), rest))
                })?;
            }
        }

        let class = input.parse()?;

        Ok(Self {
            package,
            imports,
            uses,
            class,
            errors,
        })
    }
}

impl Parse for JavaClass {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let _ = input.parse::<kw::class>()?;
        let name = input.parse()?;
        let body = input.parse()?;

        Ok(Self {
            name,
            body,
        })
    }
}

impl Parse for ClassBody {
    fn parse(body: syn::parse::ParseStream) -> syn::Result<Self> {
        let input;
        braced!(input in body);
        let mut fields = Vec::new();
        let mut bindings = Vec::new();
        let mut natives = Vec::new();
        let mut errors = Vec::new();

        fn recover(input: ParseStream) -> syn::Result<()> {
            input.step(|cursor| {
                let mut rest = *cursor;
                while let Some((tt, next)) = rest.token_tree() {
                    match tt {
                        TokenTree::Punct(p) if p.as_char() == ';' => {
                            return Ok(((), next))
                        }
                        TokenTree::Group(g) if g.delimiter() == Delimiter::Brace => {
                            return Ok(((), next))
                        }
                        _ => rest = next,
                    }
                }
                Ok(((), rest))
            })
        }

        while !input.is_empty() {
            if let Err(e) = (|| -> syn::Result<()> {
                if input.peek(kw::native) || input.peek2(kw::native) {
                    let static_tk = input.parse().ok();

                    let _ = input.parse::<kw::native>()?;
                    let signature = input.parse()?;

                    if !input.peek(syn::token::Brace) {
                        return Err(syn::Error::new(input.span(), "expected '{'"));
                    }

                    let body = input.parse()?;

                    natives.push(NativeFunction {
                        static_tk,
                        signature,
                        body,
                    });
                } else if input.peek(Token![fn]) || input.peek2(Token![fn]) {
                    let static_tk = input.parse().ok();
                    let signature = input.parse()?;
                    let _ = input.parse::<Token![;]>()?;

                    bindings.push(JavaBinding {
                        static_tk,
                        signature
                    });
                } else if input.peek(Ident) || (input.peek(Token![static]) && input.peek2(Ident)) {
                    let static_tk = input.parse().ok();
                    let name = input.parse()?;
                    let _ = input.parse::<Token![:]>()?;
                    let typ = input.parse()?;
                    let _ = input.parse::<Token![;]>()?;

                    fields.push(JavaField {
                        static_tk,
                        name,
                        typ
                    });
                } else {
                    return Err(syn::Error::new(input.span(), "expected field or function"));
                }
                Ok(())
            })() {
                errors.push(e.to_compile_error());
                recover(&input)?;
            }
        }

        Ok(Self {
            fields,
            bindings,
            natives,
            errors,
        })
    }
}

impl Parse for JavaFnSignature {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let fn_tk = input.parse()?;
        let name = input.parse()?;
        let args;
        parenthesized!(args in input);
        let params = Punctuated::parse_terminated(&args)?;

        let (arrow, returns) = if input.peek(Token![->]) {
            let arrow = input.parse()?;
            let ty = input.parse()?;
            (Some(arrow), ty)
        } else {
            (None, JavaType::Void)
        };

        Ok(Self {
            fn_tk,
            name,
            params,
            arrow,
            returns,
        })
    }
}

impl Parse for JavaParam {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name = input.parse()?;
        let _ = input.parse::<Token![:]>()?;
        let typ = input.parse()?;

        Ok(Self {
            name,
            typ
        })
    }
}

impl Parse for JavaType {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut base = 'base: {
            macro_rules! check {
                ($tk:tt, $v:tt) => {
                    if input.peek(kw::$tk) {
                        let _ = input.parse::<kw::$tk>()?;
                        break 'base Self::$v;
                    }
                };
            }

            check!(boolean, Boolean);
            check!(byte, Byte);
            check!(char, Char);
            check!(short, Short);
            check!(int, Int);
            check!(long, Long);
            check!(float, Float);
            check!(double, Double);

            let name: DottedName = input.parse()?;

            let n = name.to_string();
            if n == "String" || n == "java.lang.String" {
                JavaType::String
            } else {
                JavaType::Object(name)
            }
        };

        while input.peek(syn::token::Bracket) {
            let i;
            bracketed!(i in input);
            base = JavaType::Array(Box::new(base));
        }

        Ok(base)
    }
}

impl Parse for JavaCodeBlock {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let (delimiter, sections) = if input.peek(syn::token::Brace) {
            let inner;
            braced!(inner in input);
            (Delimiter::Brace, parse_sections(&inner, false)?)
        } else if input.peek(syn::token::Bracket) {
            let inner;
            bracketed!(inner in input);
            (Delimiter::Bracket, parse_sections(&inner, false)?)
        } else if input.peek(syn::token::Paren) {
            let inner;
            parenthesized!(inner in input);
            (Delimiter::Parenthesis, parse_sections(&inner, false)?)
        } else {
            // No delimiter: read until `;`, `,`, or the end of the current group/stream.
            let start = input.span();
            let sections = parse_sections(input, true)?;
            if sections.is_empty() {
                return Err(syn::Error::new(start, "expected an expression"));
            }
            (Delimiter::None, sections)
        };

        Ok(Self { delimiter, sections })
    }
}

fn parse_sections(input: ParseStream, stop_at_separator: bool) -> syn::Result<Vec<CodeSection>> {
    let mut sections = Vec::new();
    let mut stream = TokenStream::new();

    fn split(stream: &mut TokenStream, sects: &mut Vec<CodeSection>) {
        if !stream.is_empty() {
            sects.push(CodeSection::Tokens(std::mem::take(stream)));
        }
    }

    while !input.is_empty() {
        if stop_at_separator && (input.peek(Token![;]) || input.peek(Token![,])) {
            break;
        }

        if input.peek(Token![#])
            && !(input.peek2(syn::token::Bracket) || input.peek2(Token![!]))
        {
            split(&mut stream, &mut sections);
            let java_stmt = input.parse()?;
            sections.push(CodeSection::Java(java_stmt));
        } else if input.cursor().any_group().is_some() {
            split(&mut stream, &mut sections);
            let block = input.parse()?;
            sections.push(CodeSection::Block(block));
        } else {
            stream.extend(std::iter::once(input.parse::<TokenTree>()?));
        }
    }
    split(&mut stream, &mut sections);
    Ok(sections)
}

fn recover(input: ParseStream) -> syn::Result<TokenStream> {
    input.step(|cursor| {
        let mut stream = TokenStream::new();
        let mut rest = *cursor;
        while let Some((tt, next)) = rest.token_tree() {
            stream.append(tt.clone());
            match &tt {
                TokenTree::Punct(p) if p.as_char() == ';' => {
                    return Ok((stream, next))
                }
                _ => rest = next,
            }
        }
        return Ok((stream, rest))
    })
}

impl Parse for JavaExpression {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(kw::throw) {
            let _ = input.parse::<kw::throw>()?;
            if input.peek(kw::new) {
                let _ = input.parse::<kw::new>()?;
                let class = input.parse()?;
                let message;
                parenthesized!(message in input);
                let message = message.parse()?;
                Ok(Self::ThrowNew(JavaThrowNew {
                    class,
                    message,
                }))
            } else {
                let value = input.parse()?;
                Ok(Self::ThrowObj(JavaThrowObj {
                    value,
                }))
            }
        } else {
            let expr: JavaExpr = input.parse()?;
            if input.peek(Token![=]) {
                match expr {
                    JavaExpr::StaticFieldRead(JavaStaticFieldRead {
                        class,
                        field,
                        signature,
                    }) => {
                        let _ = input.parse::<Token![=]>()?;
                        let value = input.parse()?;
                        return Ok(Self::StaticFieldWrite(JavaStaticFieldWrite {
                            class,
                            field,
                            signature,
                            value,
                        }))
                    },
                    JavaExpr::FieldRead(JavaFieldRead {
                        obj,
                        field,
                        signature,
                    }) => {
                        let _ = input.parse::<Token![=]>()?;
                        let value = input.parse()?;
                        return Ok(Self::FieldWrite(JavaFieldWrite {
                            obj,
                            field,
                            signature,
                            value,
                        }))
                    },
                    JavaExpr::ArrayRead(ar) => {
                        let JavaArrayRead {
                            array,
                            index,
                            signature,
                        } = *ar;
                        let _ = input.parse::<Token![=]>()?;
                        let value = input.parse()?;
                        return Ok(Self::ArrayWrite(JavaArrayWrite {
                            array,
                            index,
                            signature,
                            value,
                        }))
                    },
                    _ => {}
                }
            }
            Ok(Self::Expr(expr))
        }
    }
}

impl Parse for JavaValueExpr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(Token![#]) {
            let expr = input.parse()?;
            Ok(Self::Expr(expr))
        } else {
            let block = input.parse()?;
            Ok(Self::Block(block))
        }
    }
}

impl Parse for JavaExpr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let _ = input.parse::<Token![#]>()?;
        if input.peek(kw::new) {
            let _ = input.parse::<kw::new>()?;
            let class = input.parse()?;
            let args;
            parenthesized!(args in input);
            let args = Punctuated::parse_terminated(&args)?;

            let signature = if input.peek(Token![#]) {
                Some(input.parse()?)
            } else {
                None
            };

            Ok(Self::New(JavaNew {
                class,
                args,
                signature,
            }))
        } else if input.peek(kw::alloc) {
            let _ = input.parse::<kw::alloc>()?;
            let class = input.parse()?;
            let p;
            parenthesized!(p in input);
            Ok(Self::Alloc(JavaAlloc { class }))
        } else if input.peek2(syn::token::Bracket) {
            let array = input.parse()?;
            let index;
            bracketed!(index in input);
            let index = index.parse()?;
            let signature = if input.peek(Token![#]) {
                Some(input.parse()?)
            } else {
                None
            };
            Ok(Self::ArrayRead(Box::new(JavaArrayRead {
                array,
                index,
                signature
            })))
        } else {
            // java.lang.Class::func() #["..."]
            // obj.method(...) #["..."]
            // java.lang.Class::field #["..."]
            // obj.field #["..."]
            let mut dotted: DottedName = input.parse()?;
            if input.peek(Token![::]) {
                // function or static field
                let _ = input.parse::<Token![::]>()?;
                let name = input.parse()?;
                if input.peek(syn::token::Paren) {
                    let args;
                    parenthesized!(args in input);
                    let args = Punctuated::parse_terminated(&args)?;
                    let signature = if input.peek(Token![#]) {
                        Some(input.parse()?)
                    } else {
                        None
                    };
                    Ok(Self::StaticCall(JavaStaticCall {
                        class: dotted,
                        func: name,
                        args,
                        signature,
                    }))
                } else {
                    let signature = if input.peek(Token![#]) {
                        Some(input.parse()?)
                    } else {
                        None
                    };
                    Ok(Self::StaticFieldRead(JavaStaticFieldRead {
                        class: dotted,
                        field: name,
                        signature
                    }))
                }
            } else if dotted.0.len() == 2 {
                let name = dotted.0.pop().unwrap();
                let obj = dotted.0.pop().unwrap();
                if input.peek(syn::token::Paren) {
                    // method
                    let args;
                    parenthesized!(args in input);
                    let args = Punctuated::parse_terminated(&args)?;
                    let signature = if input.peek(Token![#]) {
                        Some(input.parse()?)
                    } else {
                        None
                    };
                    Ok(Self::MethodCall(JavaMethodCall {
                        obj,
                        func: name,
                        args,
                        signature
                    }))
                } else {
                    // field
                    let signature = if input.peek(Token![#]) {
                        Some(input.parse()?)
                    } else {
                        None
                    };
                    Ok(Self::FieldRead(JavaFieldRead {
                        obj,
                        field: name,
                        signature
                    }))
                }
            } else {
                let ts = recover(input)?;
                Ok(Self::Error(ts))
            }
        }
    }
}

impl Parse for JavaSignatureTag {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let _ = input.parse::<Token![#]>()?;
        let tag_input;
        bracketed!(tag_input in input);
        let tag = tag_input.parse()?;
        Ok(Self(tag))
    }
}






