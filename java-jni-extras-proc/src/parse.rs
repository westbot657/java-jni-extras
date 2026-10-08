use std::fmt::Display;

use proc_macro2::{Delimiter, TokenStream, TokenTree};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{Ident, LitStr, Token, braced, bracketed, parenthesized};

use crate::lib2::{JavaType, kw};

pub(crate) struct DottedName(Punctuated<Ident, Token![.]>);

impl Display for DottedName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let n = self.0
            .iter()
            .map(|t| t.to_string())
            .collect::<Vec<_>>()
            .join(".");
        write!(f, "{}", n)
    }
}

pub(crate) struct JavaClassDef {
    package: DottedName,
    imports: Vec<JavaImport>,
    uses: Vec<JavaUse>,
    class: JavaClass,
    errors: Vec<TokenStream>,
}

pub(crate) struct JavaImport {
    import_tk: kw::import,
    item: DottedName,
}

pub(crate) struct JavaUse {
    use_tk: Token![use],
    rust_path: PathSepName,
    java_path: DottedName,
}

#[derive(Default)]
pub(crate) enum PathSepPrefix {
    #[default]
    None,
    This,
    Super,
    Crate,
}

pub(crate) struct PathSepName {
    prefix: PathSepPrefix,
    parts: Punctuated<Ident, Token![::]>,
}

pub(crate) struct JavaClass {
    class_tk: kw::class,
    name: Ident,
    body: ClassBody,
}

pub(crate) struct ClassBody {
    fields: Vec<JavaField>,
    bindings: Vec<JavaBinding>,
    natives: Vec<NativeFunction>,
    errors: Vec<TokenStream>,
}

pub(crate) struct JavaField {
    static_tk: Option<Token![static]>,
    name: Ident,
    typ: JavaType,
}

pub(crate) struct JavaBinding {
    static_tk: Option<Token![static]>,
    signature: JavaFnSignature,
}

pub(crate) struct NativeFunction {
    static_tk: Option<Token![static]>,
    native_tk: kw::native,
    signature: JavaFnSignature,
    body: JavaCodeBlock,
}

pub(crate) struct JavaFnSignature {
    fn_tk: Token![fn],
    name: Ident,
    params: Punctuated<JavaParam, Token![,]>,
    arrow: Option<Token![->]>,
    returns: JavaType,
}

pub(crate) struct JavaParam {
    name: Ident,
    typ: JavaType,
}

pub(crate) struct JavaCodeBlock {
    delimiter: Delimiter,
    sections: Vec<CodeSection>,
}

pub(crate) enum CodeSection {
    Tokens(TokenStream),
    Block(JavaCodeBlock),
    Java(JavaExpression),
}

pub(crate) enum JavaExpression {
    Error(TokenStream),
    StaticCall(JavaStaticCall),
    MethodCall(JavaMethodCall),
    StaticFieldWrite(JavaStaticFieldWrite),
    StaticFieldRead(JavaStaticFieldRead),
    FieldWrite(JavaFieldWrite),
    FieldRead(JavaFieldRead),
    ArrayWrite(JavaArrayWrite),
    ArrayRead(JavaArrayRead),
    ThrowNew(JavaThrowNew),
    ThrowObj(JavaThrowObj),
    New(JavaNew),
    Alloc(JavaAlloc),
}

/// matches function and field signatures:
/// #["V(Ljava/lang/String;)"]
/// #["Z"]
pub(crate) struct JavaSignatureTag(LitStr);

/// #Unknown::func(...) #["V(Ljava/lang/String;IIF)"]
pub(crate) struct JavaStaticCall {
    class: DottedName,
    func: Ident,
    args: Punctuated<JavaValueExpr, Token![,]>,
    /// May be None if the function has no args and the return type can be inferred or is void.
    signature: Option<JavaSignatureTag>,
}

/// #unknown.method(...) #["V(Ljava/lang/String;FF)"]
pub(crate) struct JavaMethodCall {
    obj: Ident,
    func: Ident,
    args: Punctuated<JavaValueExpr, Token![,]>,
    /// May be None if the function has no args and the return type can be inferred or is void.
    signature: Option<JavaSignatureTag>,
}

/// #Class::x #["J"]
pub(crate) struct JavaStaticFieldRead {
    class: Ident,
    field: Ident,
    /// only needed if the type can't be inferred.
    signature: Option<JavaSignatureTag>,
}

/// #Class::x #["Ljava/lang/String;"] = ...;
pub(crate) struct JavaStaticFieldWrite {
    class: Ident,
    field: Ident,
    /// only needed if the type can't be inferred.
    signature: Option<JavaSignatureTag>,
    value: JavaValueExpr,
}

/// #this.x #["Ljava/lang/String;"] = ...;
pub(crate) struct JavaFieldWrite {
    obj: Ident,
    field: Ident,
    /// only needed if the type can't be inferred.
    signature: Option<JavaSignatureTag>,
    value: JavaValueExpr,
}

/// #this.x #["J"]
pub(crate) struct JavaFieldRead {
    obj: Ident,
    field: Ident,
    /// only needed if the type can't be inferred.
    signature: Option<JavaSignatureTag>,
}

/// #arr[...] #["Z"] = ...;
pub(crate) struct JavaArrayWrite {
    array: Ident,
    index: JavaValueExpr,
    /// only needed if the type can't be inferred.
    signature: Option<JavaSignatureTag>,
    value: JavaValueExpr,
}

/// #arr[...] #["I"]
pub(crate) struct JavaArrayRead {
    array: Ident,
    index: JavaValueExpr,
    /// only needed if the type can't be inferred.
    signature: Option<JavaSignatureTag>,
}

/// #throw new Class(...);
pub(crate) struct JavaThrowNew {
    class: DottedName,
    message: JavaValueExpr,
}

/// #throw ...;
pub(crate) struct JavaThrowObj {
    value: JavaValueExpr,
}

/// #new Class(...) #["IJJLcom/beatcraft/interop/Note;"]
pub(crate) struct JavaNew {
    class: DottedName,
    args: Punctuated<JavaValueExpr, Token![,]>,
    /// May be None if the ctor has no args or can all be inferred.
    signature: Option<JavaSignatureTag>,
}

/// #alloc Class()
pub(crate) struct JavaAlloc {
    class: DottedName,
}

/// Matches any Java/rust expression that yields a value.
/// Block may fail to yield a valid value, but allows inline rust easier.
pub(crate) enum JavaValueExpr {
    StaticCall(JavaStaticCall),
    MethodCall(JavaMethodCall),
    StaticFieldRead(JavaStaticFieldRead),
    FieldRead(JavaFieldRead),
    ArrayRead(Box<JavaArrayRead>),
    New(JavaNew),
    Block(JavaCodeBlock),
}

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

        let parts = Punctuated::parse_terminated(input)?;

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

        let mut imports = Vec::new();
        let mut uses = Vec::new();

        let mut errors = Vec::new();

        while let is_import = input.peek(kw::import) && (is_import || input.peek(Token![use])) {
            match (|| -> syn::Result<()> {
                if is_import {
                    let import_tk = input.parse()?;
                    let item = input.parse()?;
                    let _ = input.parse::<Token![;]>()?;

                    imports.push(JavaImport {
                        import_tk,
                        item,
                    });
                } else {
                    let use_tk = input.parse()?;
                    let rust_path = input.parse()?;
                    let java_path = input.parse()?;
                    let _ = input.parse::<Token![;]>()?;

                    uses.push(JavaUse {
                        use_tk,
                        rust_path,
                        java_path,
                    })
                }
                Ok(())
            })() {
                Err(e) => {
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
                Ok(()) => {}
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
        let class_tk = input.parse()?;
        let name = input.parse()?;
        let body = input.parse()?;

        Ok(Self {
            class_tk,
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
            match (|| -> syn::Result<()> {
                if input.peek(kw::native) || input.peek2(kw::native) {
                    let static_tk = input.parse().ok();

                    let native_tk = input.parse()?;
                    let signature = input.parse()?;

                    if !input.peek(syn::token::Brace) {
                        return Err(syn::Error::new(input.span(), "expected '{'"));
                    }

                    let body = input.parse()?;

                    natives.push(NativeFunction {
                        static_tk,
                        native_tk,
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
                Err(e) => {
                    errors.push(e.to_compile_error());
                    recover(&input)?;
                }
                Ok(()) => {}
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
        let (input, delimiter) = if input.peek(syn::token::Brace) {
            let i;
            braced!(i in input);
            (i, Delimiter::Brace)
        } else if input.peek(syn::token::Bracket) {
            let i;
            bracketed!(i in input);
            (i, Delimiter::Bracket)
        } else if input.peek(syn::token::Paren) {
            let i;
            parenthesized!(i in input);
            (i, Delimiter::Parenthesis)
        } else {
            return Err(syn::Error::new(input.span(), "expected '{', '[', or '('"))
        };
//         Self::parse_unwrapped(delimiter, &input)
//     }
// }
// impl JavaCodeBlock {
//     fn parse_unwrapped(delimiter: Delimiter, input: ParseStream) -> syn::Result<Self> {
        let mut sections = Vec::new();
        let mut stream = TokenStream::new();

        fn split(stream: &mut TokenStream, sects: &mut Vec<CodeSection>) {
            if !stream.is_empty() {
                sects.push(CodeSection::Tokens(std::mem::take(stream)));
            }
        }

        while !input.is_empty() {
            if input.peek(Token![#])
                && !(input.peek2(syn::token::Bracket) || input.peek2(Token![!]))
            {
                split(&mut stream, &mut sections);
                let _ = input.parse::<Token![#]>()?;
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

        Ok(Self {
            delimiter,
            sections,
        })
    }
}

impl JavaValueExpr {
    fn maybe_parse(input: ParseStream) -> Option<syn::Result<Self>> {
        

        None
    }
}

impl Parse for JavaValueExpr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let span = input.span();
        match Self::maybe_parse(input) {
            None => Err(syn::Error::new(span, "failed to parse JavaValueExpr")),
            Some(r) => r
        }
    }
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
                return Ok(Self::ThrowNew(JavaThrowNew { class, message }))
            } else {
                let value = input.parse()?;
                return Ok(Self::ThrowObj(JavaThrowObj { value }))
            }
        }


        Err(syn::Error::new(input.span(), "failed to parse JavaExpression"))

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






