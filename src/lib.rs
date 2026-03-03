extern crate proc_macro;
use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::{Ident, LitStr, Token, Result, parse::{Parse, ParseStream}, parse_macro_input};

struct JavaClass {
    package: String,
    name: Ident,
    methods: Vec<JavaMethod>,
}

enum JavaMethod {
    Constructor {
        name: Ident,
        params: Vec<JavaParam>,
    },
    Method {
        is_static: bool,
        is_native: bool,
        return_type: JavaType,
        name: Ident,
        params: Vec<JavaParam>,
    },
}

struct JavaParam {
    ty: JavaType,
    name: Ident,
}

#[derive(Clone)]
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
    Object(String),
    Array(Box<JavaType>),
}

impl JavaType {
    fn to_jni_sig(&self) -> String {
        match self {
            JavaType::Void => "V".to_string(),
            JavaType::Boolean => "Z".to_string(),
            JavaType::Byte => "B".to_string(),
            JavaType::Char => "C".to_string(),
            JavaType::Short => "S".to_string(),
            JavaType::Int => "I".to_string(),
            JavaType::Long => "J".to_string(),
            JavaType::Float => "F".to_string(),
            JavaType::Double => "D".to_string(),
            JavaType::Object(name) => {
                let slashed = name.replace('.', "/");
                // handle common unqualified names
                let resolved = match slashed.as_str() {
                    "String" => "java/lang/String".to_string(),
                    "Object" => "java/lang/Object".to_string(),
                    other => other.to_string(),
                };
                format!("L{};", resolved)
            }
            JavaType::Array(inner) => format!("[{}", inner.to_jni_sig()),
        }
    }

    fn to_rust_type(&self) -> TokenStream2 {
        match self {
            JavaType::Void => quote! { () },
            JavaType::Boolean => quote! { bool },
            JavaType::Byte => quote! { i8 },
            JavaType::Char => quote! { u16 },
            JavaType::Short => quote! { i16 },
            JavaType::Int => quote! { i32 },
            JavaType::Long => quote! { i64 },
            JavaType::Float => quote! { f32 },
            JavaType::Double => quote! { f64 },
            JavaType::Object(name) if name == "String" => quote! { &'refs str },
            JavaType::Object(_) => quote! { jni::objects::JObject<'caller> },
            JavaType::Array(_) => quote! { jni::objects::JObject<'caller> },
        }
    }
    fn to_rust_return_type(&self) -> TokenStream2 {
        match self {
            JavaType::Void => quote! { () },
            JavaType::Boolean => quote! { bool },
            JavaType::Byte => quote! { i8 },
            JavaType::Char => quote! { u16 },
            JavaType::Short => quote! { i16 },
            JavaType::Int => quote! { i32 },
            JavaType::Long => quote! { i64 },
            JavaType::Float => quote! { f32 },
            JavaType::Double => quote! { f64 },
            JavaType::Object(name) if name == "String" => quote! { String },
            JavaType::Object(_) => quote! { jni::objects::JObject<'caller> },
            JavaType::Array(_) => quote! { jni::objects::JObject<'caller> },
        }
    }

    fn to_jvalue(&self, ident: &Ident) -> TokenStream2 {
        match self {
            JavaType::Boolean => quote! { jni::objects::JValue::Bool(#ident) },
            JavaType::Byte => quote! { jni::objects::JValue::Byte(#ident) },
            JavaType::Char => quote! { jni::objects::JValue::Char(#ident) },
            JavaType::Short => quote! { jni::objects::JValue::Short(#ident) },
            JavaType::Int => quote! { jni::objects::JValue::Int(#ident) },
            JavaType::Long => quote! { jni::objects::JValue::Long(#ident) },
            JavaType::Float => quote! { jni::objects::JValue::Float(#ident) },
            JavaType::Double => quote! { jni::objects::JValue::Double(#ident) },
            JavaType::Object(name) if name == "String" => {
                let tmp = Ident::new(&format!("__jstr_{}", ident), Span::call_site());
                quote! { jni::objects::JValue::Object(&(#tmp).into()) }
            },
            JavaType::Object(_) | JavaType::Array(_) => {
                quote! { jni::objects::JValue::Object(&#ident) }
            }
            JavaType::Void => quote! { compile_error!("void cannot be a parameter type") },
        }
    }

    fn extract_return(&self, call: TokenStream2) -> TokenStream2 {
        match self {
            JavaType::Void => quote! { #call; },
            JavaType::Boolean => quote! { #call.z() },
            JavaType::Byte => quote! { #call.b() },
            JavaType::Char => quote! { #call.c() },
            JavaType::Short => quote! { #call.s() },
            JavaType::Int => quote! { #call.i() },
            JavaType::Long => quote! { #call.j() },
            JavaType::Float => quote! { #call.f() },
            JavaType::Double => quote! { #call.d() },
            JavaType::Object(name) if name == "String" => {
                quote!( let o = #call.l()?; Ok(JString::cast_local(env, o)?.to_string()) )
            }
            JavaType::Object(_) | JavaType::Array(_) => quote! { #call.l() },
        }
    }

    fn to_jni_param_type(&self) -> TokenStream2 {
        match self {
            JavaType::Void => quote!(),
            JavaType::Boolean => quote!(boolean),
            JavaType::Byte => quote!(byte),
            JavaType::Char => quote!(char),
            JavaType::Short => quote!(short),
            JavaType::Int => quote!(int),
            JavaType::Long => quote!(jlong),
            JavaType::Float => quote!(float),
            JavaType::Double => quote!(double),
            JavaType::Object(x) => quote!(#x),
            JavaType::Array(_) => quote!(todo!()),
        }
    }
    fn to_jni_return_type(&self) -> TokenStream2 {
        match self {
            JavaType::Void => quote!(),
            JavaType::Boolean => quote!(-> boolean),
            JavaType::Byte => quote!(-> byte),
            JavaType::Char => quote!(-> char),
            JavaType::Short => quote!(-> short),
            JavaType::Int => quote!(-> int),
            JavaType::Long => quote!(-> jlong),
            JavaType::Float => quote!(-> float),
            JavaType::Double => quote!(-> double),
            JavaType::Object(x) => quote!(-> #x),
            JavaType::Array(_) => quote!(-> todo!()),
        }
    }
}

fn parse_java_type(input: ParseStream) -> Result<JavaType> {
    let ty = if input.peek(Ident) {
        let ident: Ident = input.parse()?;
        match ident.to_string().as_str() {
            "void" => JavaType::Void,
            "boolean" => JavaType::Boolean,
            "byte" => JavaType::Byte,
            "char" => JavaType::Char,
            "short" => JavaType::Short,
            "int" => JavaType::Int,
            "long" => JavaType::Long,
            "float" => JavaType::Float,
            "double" => JavaType::Double,
            other => {
                let mut name = other.to_string();
                while input.peek(Token![.]) {
                    input.parse::<Token![.]>()?;
                    let next: Ident = input.parse()?;
                    name.push('.');
                    name.push_str(&next.to_string());
                }
                JavaType::Object(name)
            }
        }
    } else {
        return Err(input.error("expected Java type"));
    };

    if input.peek(syn::token::Bracket) {
        let content;
        syn::bracketed!(content in input);
        let _ = content; // empty []
        return Ok(JavaType::Array(Box::new(ty)));
    }

    Ok(ty)
}

impl Parse for JavaMethod {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut is_static = false;
        let mut is_native = false;

        // parse modifiers
        loop {
            if input.peek(Token![static]) {
                input.parse::<Token![static]>()?;
                is_static = true;
            } else if input.peek(Ident) {
                let ident: Ident = input.fork().parse()?;
                match ident.to_string().as_str() {
                    "native" => { input.parse::<Ident>()?; is_native = true; }
                    "public" | "private" | "protected" | "final" | "synchronized" => {
                        input.parse::<Ident>()?;
                    }
                    _ => break,
                }
            } else {
                break;
            }
        }

        // peek: if next is Ident followed immediately by '(' it's a constructor
        let is_constructor = input.peek(Ident) && {
            let fork = input.fork();
            let _: Ident = fork.parse()?;
            fork.peek(syn::token::Paren)
        };

        if is_constructor {
            let name: Ident = input.parse()?;
            let content;
            syn::parenthesized!(content in input);
            let params = parse_params(&content)?;
            input.parse::<Token![;]>()?;
            return Ok(JavaMethod::Constructor { name, params });
        }

        let return_type = parse_java_type(input)?;
        let name: Ident = input.parse()?;
        let content;
        syn::parenthesized!(content in input);
        let params = parse_params(&content)?;
        input.parse::<Token![;]>()?;

        Ok(JavaMethod::Method { is_static, is_native, return_type, name, params })
    }
}

fn parse_params(content: ParseStream) -> Result<Vec<JavaParam>> {
    let mut params = Vec::new();
    while !content.is_empty() {
        let ty = parse_java_type(content)?;
        let name: Ident = content.parse()?;
        params.push(JavaParam { ty, name });
        if content.peek(Token![,]) {
            content.parse::<Token![,]>()?;
        }
    }
    Ok(params)
}

impl Parse for JavaClass {
    fn parse(input: ParseStream) -> Result<Self> {
        let pkg_kw: Ident = input.parse()?;
        if pkg_kw != "package" {
            return Err(syn::Error::new(pkg_kw.span(), "expected 'package'"));
        }

        let mut package = String::new();
        loop {
            let ident: Ident = input.parse()?;
            package.push_str(&ident.to_string());
            if input.peek(Token![;]) {
                input.parse::<Token![;]>()?;
                break;
            }
            input.parse::<Token![.]>()?;
            package.push('.');
        }

        let class_kw: Ident = input.parse()?;
        if class_kw != "class" {
            return Err(syn::Error::new(class_kw.span(), "expected 'class'"));
        }

        let name: Ident = input.parse()?;

        let content;
        syn::braced!(content in input);

        let mut methods = Vec::new();
        while !content.is_empty() {
            methods.push(content.parse::<JavaMethod>()?);
        }

        Ok(JavaClass { package, name, methods })
    }
}

fn class_path(package: &str, name: &str) -> String {
    format!("{}.{}", package, name).replace('.', "/")
}

fn generate_method(
    class_path_lit: &LitStr,
    is_static: bool,
    return_type: &JavaType,
    name: &Ident,
    params: &[JavaParam]
) -> TokenStream2 {
    let method_name = name;
    let method_name_str = method_name.to_string();

    let param_sig: String = params.iter().map(|p| p.ty.to_jni_sig()).collect();
    let return_sig = return_type.to_jni_sig();
    let full_sig = format!("({}){}", param_sig, return_sig);
    let sig_lit = LitStr::new(&full_sig, Span::call_site());
    let method_name_lit = LitStr::new(&method_name_str, Span::call_site());

    let rust_params: Vec<TokenStream2> = params.iter().map(|p| {
        let pname = &p.name;
        let pty = p.ty.to_rust_type();
        quote! { #pname: #pty }
    }).collect();

    let string_conversions: Vec<TokenStream2> = params.iter().map(|p| {
        let pname = &p.name;
        match &p.ty {
            JavaType::Object(name) if name == "String" => {
                let tmp = Ident::new(&format!("__jstr_{}", pname), Span::call_site());
                quote! { let #tmp = env.new_string(#pname)?; }
            }
            _ => quote! {},
        }
    }).collect();

    let jvalues: Vec<TokenStream2> = params.iter().map(|p| {
        p.ty.to_jvalue(&p.name)
    }).collect();

    let return_type_ts = return_type.to_rust_return_type();

    let call = if is_static {
        quote! { env.call_static_method(jni_str!(#class_path_lit), jni_str!(#method_name_lit), jni_sig!(#sig_lit), &[#(#jvalues),*])? }
    } else {
        quote! { env.call_method(obj, jni_str!(#method_name_lit), jni_sig!(#sig_lit), &[#(#jvalues),*])? }
    };

    let body = match &return_type {
        JavaType::Void => quote! {
            #(#string_conversions)*
            #call;
            Ok(())
        },
        _ => {
            let extract = return_type.extract_return(call);
            quote! {
                #(#string_conversions)*
                #extract
            }

        }
    };

    if is_static {
        quote! {
            pub fn #method_name<'caller, 'refs>(
                env: &'refs mut jni::Env<'caller>,
                #(#rust_params),*
            ) -> Result<#return_type_ts, jni::errors::Error> {
                #body
            }
        }
    } else {
        quote! {
            pub fn #method_name<'caller, 'refs>(
                env: &'refs mut jni::Env<'caller>,
                obj: &'refs jni::objects::JObject<'caller>,
                #(#rust_params),*
            ) -> Result<#return_type_ts, jni::errors::Error> {
                #body
            }
        }
    }
}

fn generate_constructor(
    class_path_lit: &LitStr,
    name: &Ident,
    params: &[JavaParam],
) -> TokenStream2 {
    let param_sig: String = params.iter().map(|p| p.ty.to_jni_sig()).collect();
    let sig_lit = LitStr::new(&format!("({})V", param_sig), Span::call_site());

    let rust_params: Vec<TokenStream2> = params.iter().map(|p| {
        let pname = &p.name;
        let pty = p.ty.to_rust_type();
        quote! { #pname: #pty }
    }).collect();

    let string_conversions: Vec<TokenStream2> = params.iter().map(|p| {
        let pname = &p.name;
        match &p.ty {
            JavaType::Object(n) if n == "String" => {
                let tmp = Ident::new(&format!("__jstr_{}", pname), Span::call_site());
                quote! { let #tmp = env.new_string(#pname)?; }
            }
            _ => quote! {},
        }
    }).collect();

    let jvalues: Vec<TokenStream2> = params.iter().map(|p| {
        p.ty.to_jvalue(&p.name)
    }).collect();

    quote! {
        pub fn #name<'caller>(
            env: &mut jni::Env<'caller>,
            #(#rust_params),*
        ) -> Result<jni::objects::JObject<'caller>, jni::errors::Error> {
            #(#string_conversions)*
            env.new_object(
                jni_str!(#class_path_lit),
                jni_sig!(#sig_lit),
                &[#(#jvalues),*]
            )
        }
    }
}

#[proc_macro]
pub fn java_class_decl(input: TokenStream) -> TokenStream {
    let class = parse_macro_input!(input as JavaClass);

    let struct_name = &class.name;
    let cp = class_path(&class.package, &class.name.to_string());
    let class_path_lit = LitStr::new(&cp, Span::call_site());

    let native_registrations: Vec<TokenStream2> = class.methods.iter()
        .filter_map(|m| match m {
            JavaMethod::Method { is_native: true, name, params, return_type, is_static, .. } => {

                let package_class = format!("{}.{}", class.package, class.name);
                let package_class_lit = LitStr::new(&package_class, Span::call_site());
                let return_type = return_type.to_jni_return_type();
                let params: Vec<TokenStream2> = params.iter().map(|p| {
                    p.ty.to_jni_param_type()
                }).collect();

                Some(if *is_static {
                    quote! {
                        const _: jni::NativeMethod = jni::native_method! {
                            java_type = #package_class_lit,
                            static extern fn #name(#(#params),*) #return_type,
                        };
                    }
                } else {
                    quote! {
                        const _: jni::NativeMethod = jni::native_method! {
                            java_type = #package_class_lit,
                            extern fn #name(#(#params),*) #return_type,
                        };
                    }
                })
            }
            _ => None,
        })
        .collect();

    let validate_checks: Vec<TokenStream2> = class.methods.iter()
        .filter_map(|m| match m {
            JavaMethod::Method { is_native: false, name, params, return_type, is_static, .. } => {

                let method_name_str = name.to_string();
                let method_name_lit = LitStr::new(&method_name_str, Span::call_site());
                let param_sig: String = params.iter().map(|p| p.ty.to_jni_sig()).collect();
                let return_sig = return_type.to_jni_sig();
                let sig_lit = LitStr::new(&format!("({}){}", param_sig, return_sig), Span::call_site());
                let class_path_lit_str = LitStr::new(&format!("{}.{}", class.package, class.name), Span::call_site());

                Some(if *is_static {
                    quote! {
                        env.get_static_method_id(
                            jni_str!(#class_path_lit_str),
                            jni_str!(#method_name_lit),
                            jni_sig!(#sig_lit),
                        )?;
                    }
                } else {
                    quote! {
                        env.get_method_id(
                            jni_str!(#class_path_lit_str),
                            jni_str!(#method_name_lit),
                            jni_sig!(#sig_lit),
                        )?;
                    }
                })
            }
            _ => None,
        })
        .collect();

    let methods: Vec<TokenStream2> = class.methods.iter()
        .filter_map(|m| match m {
            JavaMethod::Constructor { name, params } => {
                Some(generate_constructor(&class_path_lit, name, params))
            }
            JavaMethod::Method { is_static, is_native: false, return_type, name, params, } => {
                Some(generate_method(&class_path_lit, *is_static, return_type, name, params))
            }
            _ => None,
        })
        .collect();


    let expanded = quote! {
        #(#native_registrations)*

        pub struct #struct_name;

        impl #struct_name {
            pub fn _validate_interface(env: &mut jni::Env<'_>) -> Result<(), jni::errors::Error> {
                #(#validate_checks)*
                Ok(())
            }

            #(#methods)*
        }
    };

    expanded.into()
}