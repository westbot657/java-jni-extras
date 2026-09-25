use proc_macro::TokenStream;
use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::ops::Deref;
use quote::{quote, ToTokens, TokenStreamExt, format_ident};
use syn::{parse_macro_input, Block, Ident, Token, ItemUse, Error, braced, parenthesized, bracketed, parse_str, Type, parse_quote, Path};
use syn::parse::{Parse, ParseStream};

#[proc_macro]
pub fn java_class(input: TokenStream) -> TokenStream {
    let mut java_mod = parse_macro_input!(input as JavaMod);
    java_mod.resolve();
    quote!{ #java_mod }.into()
}

// output structs
struct JavaMod {
    package: DottedName,
    import_paths: JavaImports,
    class: JavaClass,
}

#[derive(Default)]
struct JavaImports {
    java_types: HashMap<String, DottedName>,
    rust_types: HashMap<String, ScopedName>,
}

struct JavaClass {
    name: Ident,
    body: JavaClassBody,
}

struct JavaClassBody {
    natives: Vec<NativeFunction>,
    externs: Vec<ExternFunction>,
}

struct NativeFunction {
    is_static: bool,
    signature: JavaSignature,
    body: Block
}

struct ExternFunction {
    is_static: bool,
    signature: JavaSignature,
}

struct JavaSignature {
    name: Ident,
    args: Vec<JavaArg>,
    returns: JavaType,
}

struct JavaArg {
    name: Ident,
    typ: JavaType,
}

#[derive(PartialEq)]
enum JavaType {
    Void,
    Boolean,
    Byte,
    Char,
    Short,
    Int,
    Long,
    Float,
    Double,
    String,
    Object(DottedName),
    Array(Box<Self>),
}

// parse helper structs
#[derive(Clone, Debug, PartialEq)]
struct DottedName(Vec<Ident>);
impl Parse for DottedName {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut names = Vec::new();
        loop {
            let name: Ident = input.parse()?;
            names.push(name);
            if input.peek(Token![.]) {
                let _ = input.parse::<Token![.]>()?;
            } else {
                break Ok(Self(names))
            }
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum Prefix {
    Crate,
    This,
    Super,
    None,
}
#[derive(Clone, Debug)]
struct ScopedName(Prefix, Vec<Ident>);
impl Parse for ScopedName {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut names = Vec::new();
        let prefix = if input.peek(Token![self]) {
            let _ = input.parse::<Token![self]>()?;
            let _ = input.parse::<Token![::]>()?;
            Prefix::This
        } else if input.peek(Token![super]) {
            let _ = input.parse::<Token![super]>()?;
            let _ = input.parse::<Token![::]>()?;
            Prefix::Super
        } else if input.peek(Token![crate]) {
            let _ = input.parse::<Token![crate]>()?;
            let _ = input.parse::<Token![::]>()?;
            Prefix::Crate
        } else {
            Prefix::None
        };
        loop {
            if input.peek(Ident) && input.peek2(Token![::]) {
                let name: Ident = input.parse()?;
                let _ = input.parse::<Token![::]>()?;
                names.push(name);
            } else {
                break Ok(Self(prefix, names))
            }
        }
    }
}

mod kw {
    syn::custom_keyword!(class);
    syn::custom_keyword!(package);
    syn::custom_keyword!(native);
    syn::custom_keyword!(import);
    syn::custom_keyword!(void);
    syn::custom_keyword!(boolean);
    syn::custom_keyword!(byte);
    syn::custom_keyword!(char);
    syn::custom_keyword!(short);
    syn::custom_keyword!(int);
    syn::custom_keyword!(long);
    syn::custom_keyword!(float);
    syn::custom_keyword!(double);
}

// state
impl DottedName {
    fn first(&self) -> &Ident {
        self.0.first().unwrap()
    }
    fn last(&self) -> &Ident {
        self.0.last().unwrap()
    }
    fn last_mut(&mut self) -> &mut Ident {
        self.0.last_mut().unwrap()
    }
    fn is_dotted(&self) -> bool {
        self.0.len() > 1
    }
    fn class_path(&self) -> String {
        format!("L{};", self.partial_class_path())
    }
    fn partial_class_path(&self) -> String {
        self.0.iter().map(ToString::to_string).collect::<Vec<String>>().join("/")
    }
}
impl Display for DottedName {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.iter().map(ToString::to_string).collect::<Vec<String>>().join("."))
    }
}
impl ScopedName {
    fn push(&mut self, name: Ident) {
        self.1.push(name);
    }
    fn last_mut(&mut self) -> &mut Ident {
        self.1.last_mut().unwrap()
    }
}
impl JavaImports {
    fn add(&mut self, import: DottedName) {
        let name = import.last().to_string();
        self.java_types.insert(name, import);
    }
    fn add_use(&mut self, scoped: ScopedName, import: DottedName) {
        let name = import.last().to_string();
        self.java_types.insert(name.clone(), import);
        self.rust_types.insert(name, scoped);
    }
    fn resolve_rust_type(&self, o: &DottedName) -> Type {
        if o.to_string() == "java.lang.Object" {
            parse_quote!(jni::objects::JObject<'c>)
        } else {
            let n = o.last();
            let nn = n.to_string();
            if let Some(rt) = self.rust_types.get(&nn) {
                let mut rt = rt.clone();
                *rt.last_mut() = n.clone();
                parse_quote!(#rt<'c>)
            } else {
                parse_quote!(jni::objects::JObject<'c>)
            }
        }
    }
}

// Primary parsers
impl Parse for JavaMod {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let _ = input.parse::<kw::package>()?;
        let package: DottedName = input.parse()?;
        let _ = input.parse::<Token![;]>()?;

        let mut import_paths = JavaImports::default();
        loop {
            if input.peek(kw::import) {
                let _ = input.parse::<kw::import>()?;
                let io: DottedName = input.parse()?;
                let _ = input.parse::<Token![;]>()?;
                import_paths.add(io);
            } else if input.peek(Token![use]) {
                let _ = input.parse::<Token![use]>()?;
                let mut scoped: ScopedName = input.parse()?;
                let path: DottedName = input.parse()?;
                let _ = input.parse::<Token![;]>()?;
                scoped.push(path.last().clone());
                import_paths.add_use(scoped, path);
            } else {
                break
            }
        }

        let class: JavaClass = input.parse()?;

        Ok(Self {
            package,
            import_paths,
            class,
        })
    }
}

impl Parse for JavaClass {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let _ = input.parse::<kw::class>()?;
        let name: Ident = input.parse()?;
        let body;
        braced!(body in input);
        let body: JavaClassBody = body.parse()?;
        Ok(Self {
            name,
            body,
        })
    }
}

impl Parse for JavaClassBody {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut natives = Vec::new();
        let mut externs = Vec::new();
        while !input.is_empty() {
            if input.peek(kw::native) || input.peek2(kw::native) {
                let native: NativeFunction = input.parse()?;
                natives.push(native);
            } else {
                let ext: ExternFunction = input.parse()?;
                externs.push(ext);
            }
        }

        Ok(Self {
            natives,
            externs,
        })
    }
}

impl Parse for NativeFunction {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let is_static = if input.peek(Token![static]) {
            let _ = input.parse::<Token![static]>()?;
            let _ = input.parse::<kw::native>()?;
            true
        } else {
            let _ = input.parse::<kw::native>()?;
            false
        };

        let signature: JavaSignature = input.parse()?;

        let body: Block = input.parse()?;

        Ok(Self {
            is_static,
            signature,
            body,
        })
    }
}
impl Parse for ExternFunction {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let is_static = if input.peek(Token![static]) {
            let _ = input.parse::<Token![static]>()?;
            true
        } else {
            false
        };

        let signature: JavaSignature = input.parse()?;

        let _ = input.parse::<Token![;]>()?;

        Ok(Self {
            is_static,
            signature,
        })
    }
}
impl Parse for JavaSignature {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let _ = input.parse::<Token![fn]>()?;
        let name: Ident = input.parse()?;
        let raw_args;
        parenthesized!(raw_args in input);
        let mut args = Vec::new();

        while !raw_args.is_empty() {
            let name: Ident = raw_args.parse()?;
            let _ = raw_args.parse::<Token![:]>()?;
            let typ: JavaType = raw_args.parse()?;
            args.push(JavaArg { name, typ });
            if raw_args.peek(Token![,]) {
                let _ = raw_args.parse::<Token![,]>()?;
            }
        }

        let returns = if input.peek(Token![->]) {
            let _ = input.parse::<Token![->]>()?;
            input.parse()?
        } else {
            JavaType::Void
        };

        Ok(Self {
            name,
            args,
            returns,
        })
    }
}

impl Parse for JavaType {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        #[allow(unused_labels)]
        let mut base = 'base: {
            macro_rules! check {
                ($kw:tt, $typ:tt) => {
                    if input.peek(kw::$kw) { let _ = input.parse::<kw::$kw>()?; break 'base Self::$typ }
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

// Primary output
impl JavaMod {
    fn resolve(&mut self) {
        self.import_paths.add(parse_str::<DottedName>(&format!("{}.{}", self.package, self.class.name)).unwrap());
        self.import_paths.add(parse_str::<DottedName>("java.lang.Object").unwrap());
        self.import_paths.add(parse_str::<DottedName>("java.lang.String").unwrap());

        self.class.body.resolve(&self.import_paths);
    }
}
impl JavaClassBody {
    fn resolve(&mut self, imports: &JavaImports) {
        for ext in self.externs.iter_mut() {
            ext.signature.resolve(imports);
        }
        for nat in self.natives.iter_mut() {
            nat.signature.resolve(imports);
        }
    }
}
impl JavaSignature {
    fn resolve(&mut self, imports: &JavaImports) {
        for arg in self.args.iter_mut() {
            arg.typ.resolve(imports);
        }
        self.returns.resolve(imports);
    }
    fn jni_sig(&self) -> String {
        let args = self.args.iter().map(|t| t.typ.signature()).collect::<Vec<_>>().join("");
        let ret = self.returns.signature();
        format!("({args}){ret}")
    }
}
impl JavaType {
    fn resolve(&mut self, imports: &JavaImports) {
        if let Self::Object(name) = self {
            if !name.is_dotted() {
                let n = name.first().to_string();
                if let Some(path) = imports.java_types.get(&n) {
                    let l = name.0.pop().unwrap();
                    *name = (*path).clone();
                    *name.last_mut() = l;
                }
            }
        } else if let Self::Array(a) = self {
            a.resolve(imports);
        }
    }
    fn is_primitive(&self) -> bool {
        !matches!(self, Self::String | Self::Object(_) | Self::Array(_))
    }

    fn to_rust_return_type(&self, imports: &JavaImports) -> Type {
        match self {
            JavaType::Void => parse_quote!(()),
            JavaType::Boolean => parse_quote!(bool),
            JavaType::Byte => parse_quote!(i8),
            JavaType::Char => parse_quote!(u16),
            JavaType::Short => parse_quote!(i16),
            JavaType::Int => parse_quote!(i32),
            JavaType::Long => parse_quote!(i64),
            JavaType::Float => parse_quote!(f32),
            JavaType::Double => parse_quote!(f64),
            JavaType::String => parse_quote!(jni::objects::JString<'c>),
            JavaType::Object(o) => {
                imports.resolve_rust_type(o)
            }
            JavaType::Array(a) => {
                let at = a.to_rust_type(imports);
                parse_quote!(jni::objects::#at)
            }
        }
    }
    fn to_rust_type(&self, imports: &JavaImports) -> Type {
        match self {
            JavaType::Void => parse_quote!(()),
            JavaType::Boolean => parse_quote!(impl JavaTyped<'c, JType=jni::sys::jboolean>),
            JavaType::Byte => parse_quote!(impl JavaTyped<'c, JType=jni::sys::jbyte>),
            JavaType::Char => parse_quote!(impl JavaTyped<'c, JType=jni::sys::jchar>),
            JavaType::Short => parse_quote!(impl JavaTyped<'c, JType=jni::sys::jshort>),
            JavaType::Int => parse_quote!(impl JavaTyped<'c, JType=jni::sys::jint>),
            JavaType::Long => parse_quote!(impl JavaTyped<'c, JType=jni::sys::jlong>),
            JavaType::Float => parse_quote!(impl JavaTyped<'c, JType=jni::sys::jfloat>),
            JavaType::Double => parse_quote!(impl JavaTyped<'c, JType=jni::sys::jdouble>),
            JavaType::String => parse_quote!(impl JavaTyped<'c, JType=jni::objects::JString<'c>>),
            JavaType::Object(o) => {
                imports.resolve_rust_type(o)
            }
            JavaType::Array(a) => {
                let at = a.to_rust_concrete_type(imports);
                if a.is_primitive() {
                    parse_quote!(impl JavaTyped<'c, JType=JPrimitiveArray<'c, #at>>)
                } else {
                    parse_quote!(impl JavaTyped<'c, JType=JObjectArray<'c, #at>>)
                }
            }
        }
    }
    fn to_rust_concrete_type(&self, imports: &JavaImports) -> Type {
        match self {
            JavaType::Void => parse_quote!(()),
            JavaType::Boolean => parse_quote!(jni::sys::jboolean),
            JavaType::Byte => parse_quote!(jni::sys::jbyte),
            JavaType::Char => parse_quote!(jni::sys::jchar),
            JavaType::Short => parse_quote!(jni::sys::jshort),
            JavaType::Int => parse_quote!(jni::sys::jint),
            JavaType::Long => parse_quote!(jni::sys::jlong),
            JavaType::Float => parse_quote!(jni::sys::jfloat),
            JavaType::Double => parse_quote!(jni::sys::jdouble),
            JavaType::String => parse_quote!(jni::objects::JString<'c>),
            JavaType::Object(o) => {
                imports.resolve_rust_type(o)
            }
            JavaType::Array(a) => {
                let at = a.to_rust_type(imports);
                if a.is_primitive() {
                    parse_quote!(JPrimitiveArray<'c, #at>)
                } else {
                    parse_quote!(JObjectArray<'c, #at>)
                }
            }
        }
    }
}
impl JavaArg {
    fn convert_rust_to_java(&self, _imports: &JavaImports) -> proc_macro2::TokenStream {
        let name = &self.name;
        quote! {
            let #name = JavaTyped::into_java(#name, env)?;
        }
    }
    fn to_rust_arg(&self, imports: &JavaImports) -> RustArg {
        let typ: Type = self.typ.to_rust_type(imports);

        RustArg {
            name: self.name.clone(),
            typ,
        }
    }
}
struct RustArg {
    name: Ident,
    typ: Type,
}

impl ToTokens for JavaMod {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let name = &self.class.name;
        let package_class = format!("{}.{}", self.package, name);
        let class_path = format!("L{}/{};", self.package.partial_class_path(), name);

        let native_registrations = if self.class.body.natives.is_empty() {
            quote!()
        } else {
            let mut nats = proc_macro2::TokenStream::default();
            for nat in self.class.body.natives.iter() {
                let name = &nat.signature.name;
                let stat = if nat.is_static {
                    quote!(static)
                } else {
                    quote!()
                };
                let arg_types = nat.signature.args
                    .iter()
                    .map(|t|
                        parse_str::<proc_macro2::TokenStream>(&t.typ.jtype().unwrap())
                            .map_err(|e| e.to_compile_error())
                            .unwrap_or_else(|t| t))
                    .collect::<Vec<_>>();
                let ret = nat.signature.returns.jtype()
                    .map(|t| {
                        let t = parse_str::<proc_macro2::TokenStream>(&t)
                            .map_err(|e| e.to_compile_error())
                            .unwrap_or_else(|t| t);
                        quote!(-> #t)
                    }
                    ).unwrap_or_default();
                nats.append_all(quote! {
                    const _: jni::NativeMethod = jni::native_method!{
                        java_type = #package_class, #stat extern fn #name(#(#arg_types),*)#ret,
                    };
                });
            }
            nats
        };

        let (validation_checks, methods) = if self.class.body.externs.is_empty() {
            (quote!(), quote!())
        } else {
            let mut exts = proc_macro2::TokenStream::default();
            let mut methods = proc_macro2::TokenStream::default();

            for ext in self.class.body.externs.iter() {
                let name = ext.signature.name.to_string();
                let sig = ext.signature.jni_sig();
                if ext.is_static {
                    exts.append_all(quote! {
                        env.get_static_method_id(
                            jni::jni_str!(#package_class),
                            jni::jni_str!(#name),
                            jni::jni_sig!(#sig),
                        )?;
                    });
                } else {
                    exts.append_all(quote! {
                        env.get_method_id(
                            jni::jni_str!(#package_class),
                            jni::jni_str!(#name),
                            jni::jni_sig!(#sig),
                        )?;
                    });
                }
                let method = {
                    let name = &ext.signature.name;
                    let sig = ext.signature.jni_sig();
                    let display_name = name.to_string();
                    let mut conversions = Vec::new();
                    let mut params = Vec::new();
                    let args = ext.signature.args
                        .iter()
                        .map(|a| {
                            conversions.push(a.convert_rust_to_java(&self.import_paths));
                            let name = &a.name;
                            params.push(if a.typ.is_primitive() {
                                quote! { #name.into() }
                            } else {
                                quote! { (&#name).into() }
                            });
                            a.to_rust_arg(&self.import_paths)
                        });

                    let ret = ext.signature.returns.to_rust_return_type(&self.import_paths);

                    let capt = if ext.signature.returns == JavaType::Void {
                        quote!()
                    } else {
                        quote!(let ret = )
                    };

                    let ret_conv = match &ext.signature.returns {
                        JavaType::Void => quote!(Ok(())),
                        JavaType::Boolean => quote! { ret.z() },
                        JavaType::Byte => quote! { ret.b() },
                        JavaType::Char => quote! { ret.c() },
                        JavaType::Short => quote! { ret.s() },
                        JavaType::Int => quote! { ret.i() },
                        JavaType::Long => quote! { ret.j() },
                        JavaType::Float => quote! { ret.f() },
                        JavaType::Double => quote! { ret.d() },
                        JavaType::String => quote! { jni::objects::JString::cast_local(env, ret.l()?) },
                        JavaType::Object(o) => {
                            if o.to_string() == "java.lang.Object" {
                                quote!(ret.l())
                            } else {
                                let n = o.last();
                                let nn = n.to_string();
                                if let Some(rt) = self.import_paths.rust_types.get(&nn) {
                                    let mut rt = rt.clone();
                                    *rt.last_mut() = n.clone();
                                    quote! { Ok(#rt(ret.l()?)) }
                                } else {
                                    quote! { ret.l() }
                                }
                            }
                        },
                        JavaType::Array(a) => {

                            quote! {
                                ret.l()
                            }
                        },
                    };

                    if ext.is_static {
                        quote! {
                            pub fn #name<'c, 'r>(env: &'r mut jni::Env<'c>, #(#args),*) -> Result<#ret, jni::errors::Error> {
                                #(#conversions)*
                                #capt env.call_static_method(
                                    jni::jni_str!(#package_class),
                                    jni::jni_str!(#display_name),
                                    jni::jni_sig!(#sig),
                                    &[#(#params),*]
                                )?;
                                #ret_conv
                            }
                        }
                    } else {
                        quote! {

                        }
                    }
                };
                methods.append_all(method);
            }

            (exts, methods)
        };

        tokens.append_all(quote! {
            #native_registrations
            #[derive(Default)]
            pub struct #name<'local>(jni::objects::JObject<'local>);
            impl<'local> JavaTyped<'local> for #name<'local> {
                type JType = jni::objects::JObject<'local>;
                fn into_java<'r>(self, env: &'r mut jni::Env<'local>) -> jni::errors::Result<Self::JType> {
                    Ok(self.0)
                }
            }
            impl<'local> std::ops::Deref for #name<'local> {
                type Target = jni::objects::JObject<'local>;
                fn deref(&self) -> &Self::Target {
                    &self.0
                }
            }
            unsafe impl jni::refs::Reference for #name<'_> {
                type Kind<'env> = #name<'env>;
                type GlobalKind = #name<'static>;
                fn as_raw(&self) -> jni::sys::jobject { self.0.as_raw() }
                fn class_name() -> std::borrow::Cow<'static, jni::strings::JNIStr> {
                    std::borrow::Cow::Borrowed(jni::jni_str!(#package_class))
                }
                fn lookup_class<'caller>(
                    env: &jni::Env<'_>,
                    loader_context: &jni::refs::LoaderContext
                ) -> jni::errors::Result<impl std::ops::Deref<Target = jni::refs::Global<jni::objects::JClass<'static>>> + 'caller> {
                    jni::objects::JObject::lookup_class(env, loader_context)
                }
            }
            impl<'l> AsRef<jni::objects::JObject<'l>> for #name<'l> {
                fn as_ref(&self) -> &jni::objects::JObject<'l> { &self.0 }
            }
            impl<'l> From<#name<'l>> for jni::objects::JObject<'l> {
                fn from(value: #name<'l>) -> jni::objects::JObject<'l> {
                    value.0
                }
            }
            impl<'local> #name<'local> {
                pub const JAVA_SIGNATURE: jni::signature::FieldSignature<'static> = jni::jni_sig!(#class_path);
                pub fn _validate_interface(env: &mut jni::Env<'_>) -> jni::errors::Result<()> {
                    #validation_checks
                    Ok(())
                }

                #methods
            }
        });
    }
}

impl ToTokens for RustArg {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let Self { name, typ } = &self;
        tokens.append_all(quote! {
            #name: #typ
        })
    }
}

impl ToTokens for ScopedName {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let names = &self.1;
        let prefix = match self.0 {
            Prefix::Crate => quote!(crate::),
            Prefix::This => quote!(self::),
            Prefix::Super => quote!(super::),
            Prefix::None => quote!()
        };
        tokens.append_all(quote! {
            #prefix #(#names)::*
        })
    }
}

impl JavaType {
    fn type_name(&self) -> Cow<'static, str> {
        Cow::Borrowed(match self {
            JavaType::Void => "void",
            JavaType::Boolean => "boolean",
            JavaType::Byte => "byte",
            JavaType::Char => "char",
            JavaType::Short => "short",
            JavaType::Int => "int",
            JavaType::Long => "long",
            JavaType::Float => "float",
            JavaType::Double => "double",
            JavaType::String => "java.lang.String",
            JavaType::Object(o) => return Cow::Owned(o.to_string()),
            JavaType::Array(a) => return Cow::Owned(format!("{}[]", a.type_name()))
        })
    }
    fn jtype(&self) -> Option<Cow<'static, str>> {
        Some(Cow::Borrowed(match self {
            JavaType::Void => return None,
            JavaType::Boolean => "boolean",
            JavaType::Byte => "byte",
            JavaType::Char => "char",
            JavaType::Short => "short",
            JavaType::Int => "int",
            JavaType::Long => "jlong",
            JavaType::Float => "float",
            JavaType::Double => "double",
            JavaType::String => "java.lang.String",
            JavaType::Object(o) => return Some(Cow::Owned(o.to_string())),
            JavaType::Array(a) => return Some(Cow::Owned(format!("{}[]", a.type_name())))
        }))
    }
    fn signature(&self) -> Cow<'static, str> {
        Cow::Borrowed(match self {
            JavaType::Void => "V",
            JavaType::Boolean => "Z",
            JavaType::Byte => "B",
            JavaType::Char => "C",
            JavaType::Short => "S",
            JavaType::Int => "I",
            JavaType::Long => "J",
            JavaType::Float => "F",
            JavaType::Double => "D",
            JavaType::String => "Ljava/lang/String;",
            JavaType::Object(o) => return Cow::Owned(o.class_path()),
            JavaType::Array(a) => return Cow::Owned(format!("[{}", a.signature()))
        })
    }
}