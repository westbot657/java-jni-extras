use std::borrow::Cow;

use proc_macro2::{Delimiter, TokenStream};
use quote::{ToTokens, TokenStreamExt, quote};
use syn::spanned::Spanned;
use syn::{Type, parse_quote};

use crate::{DottedName, JavaAlloc, JavaArrayRead, JavaArrayWrite, JavaBinding, JavaCodeBlock, JavaExpr, JavaExpression, JavaFieldRead, JavaFieldWrite, JavaFnSignature, JavaMethodCall, JavaNew, JavaParam, JavaSignatureTag, JavaStaticCall, JavaStaticFieldRead, JavaStaticFieldWrite, JavaThrowNew, JavaThrowObj, JavaType, JavaUse, JavaValueExpr, NativeFunction, PathSepName};
use crate::process::JniClass;

impl ToTokens for JniClass {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let Self {
            uses,
            struct_name,
            class_name,
            classpath,
            package: _,
            fields: _,
            bindings,
            natives,
            errors,
        } = self;

        let native_registrations = if self.natives.is_empty() {
            quote!()
        } else {
            let mut nats = TokenStream::new();
            for nat in natives.iter() {
                nat.output(&mut nats, class_name, uses);
            }
            quote! {
                const _: () = const {
                    #nats
                };
            }
        };

        let (validation_checks, methods) = if bindings.is_empty() {
            (quote!(), quote!())
        } else {
            let mut checks = TokenStream::new();
            let mut methods = TokenStream::new();

            for JavaBinding {
                static_tk,
                signature,
            } in bindings.iter() {
                let JavaFnSignature {
                    fn_tk: _,
                    name,
                    params,
                    arrow: _,
                    returns,
                } = signature;

                let sig = signature.jni_signature();

                checks.append_all(if static_tk.is_some() {
                    quote! {
                        env.get_static_method_id(
                            jni::jni_str!(#class_name),
                            jni::jni_str!(#name),
                            jni::jni_sig!(#sig),
                        )?;
                    }
                } else {
                    quote! {
                        env.get_method_id(
                            jni::jni_str!(#class_name),
                            jni::jni_str!(#name),
                            jni::jni_str!(#sig),
                        )?;
                    }
                });
                let method = {
                    let name_str = name.to_string();
                    let mut conversions = Vec::new();
                    let mut jni_params = Vec::new();
                    let args = params
                        .iter()
                        .map(|a| {
                            conversions.push(a.rust_to_java());
                            let name = &a.name;
                            jni_params.push(if a.typ.is_primitive() {
                                quote!(#name.into())
                            } else {
                                quote!((&#name).into())
                            });
                            a.to_rust_arg(uses)
                        });

                    let ret = returns.to_rust_return_type(uses);

                    let capt = if *returns == JavaType::Void {
                        quote!()
                    } else {
                        quote!(let ret = )
                    };

                    let ret_conv = returns.jni_conversion(uses);

                    if static_tk.is_some() {
                        quote! {
                            pub fn #name<'r, 'c>(env: &'r mut jni::Env<'c>, #(#args),*) -> jni::errors::Result<#ret> {
                                #(#conversions)*
                                #capt env.call_static_method(
                                    jni::jni_str!(#class_name),
                                    jni::jni_str!(#name_str),
                                    jni::jni_sig!(#sig),
                                )?;
                                #ret_conv
                            }
                        }
                    } else {
                        quote! {
                            pub fn #name<'r, 'c>(&self, env: &'r mut jni::Env<'c>, #(#args),*) -> jni::errors::Result<#ret> {
                                #(#conversions)*
                                #capt = env.call_method(
                                    &self.0,
                                    jni::jni_str!(#name_str),
                                    jni::jni_sig!(#sig),
                                )?;
                                #ret_conv
                            }
                        }
                    }
                };
                methods.append_all(method);
            }

            (checks, methods)
        };

        tokens.append_all(quote! {
            #native_registrations
            #[derive(Default)]
            pub struct #struct_name<'local>(jni::objects::JObject<'local>);
            impl<'local> java_jni_extras::JavaTyped<'local> for #struct_name<'local> {
                type JType = jni::objects::JObject<'local>;
                fn into_java<'r>(self, env: &'r mut jni::Env<'local>) -> jni::errors::Result<Self::JType> {
                    Ok(self.0)
                }
            }
            impl<'local> std::ops::Deref for #struct_name<'local> {
                type Target = jni::objects::JObject<'local>;
                fn deref(&self) -> &Self::Target {
                    &self.0
                }
            }
            unsafe impl jni::refs::Reference for #struct_name<'_> {
                type Kind<'env> = #struct_name<'env>;
                type GlobalKind = #struct_name<'static>;
                fn as_raw(&self) -> jni::sys::jobject { self.0.as_raw() }
                fn class_name() -> std::borrow::Cow<'static, jni::strings::JNIStr> {
                    std::borrow::Cow::Borrowed(jni::jni_str!(#class_name))
                }
                fn lookup_class<'class>(
                    env: &jni::Env<'_>,
                    loader_context: &jni::refs::LoaderContext,
                ) -> jni::errors::Result<impl std::ops::Deref<Target = jni::refs::Global<jni::objects::JClass<'static>>> + 'caller> {
                    jni::objects::JObject::lookup_class(env, loader_context)
                }
            }
            impl<'l> AsRef<jni::objects::JObject<'l>> for #struct_name<'l> {
                fn as_ref(&self) -> &jni::objects::JObject<'l> { &self.0 }
            }
            impl<'l> From<#struct_name<'l>> for jni::objects::JObject<'l> {
                fn from(value: #struct_name<'l>) -> Self {
                    value.0
                }
            }
            impl<'l> From<jni::objects::JObject<'l>> for #struct_name<'l> {
                fn from(value: jni::objects::JObject<'l>) -> Self {
                    Self(value)
                }
            }
            impl<'local> #struct_name<'local> {
                pub const JAVA_SIGNATURE: jni::signature::FieldSignature<'static> = jni::jni_sig!(#classpath);
                pub fn _validate_interface(env: &mut jni::Env<'_>) -> jni::errors::Result<()> {
                    #validation_checks
                    Ok(())
                }

                #methods
            }

            #(#errors)*
        });

    }
}


impl NativeFunction {
    fn output(&self, nats: &mut TokenStream, class_name: &str, uses: &[JavaUse]) {
        let Self {
            static_tk,
            signature,
            body,
        } = self;
        let JavaFnSignature {
            fn_tk,
            name,
            params,
            arrow,
            returns,
        } = signature;

        let body = body.output(uses);

        let body = if *returns == JavaType::Void {
            quote! {
                { {#body}; Ok(()) }
            }
        } else if returns.is_primitive() {
            quote! {
                {
                    let x = {#body};
                    Ok(x)
                }
            }
        } else if returns.is_custom_object() {
            quote! {
                {
                    let v = {#body};
                    Ok(v.into_java(env)?.into())
                }
            }
        } else {
            quote! {
                {
                    let v = {#body};
                    v.into_java(env)
                }
            }
        };

        let args = params
            .iter()
            .map(|a| {
                let name = &a.name;
                let typ = a.typ.to_rust_concrete_type(uses);
                quote! {
                    #name: #typ
                }
            });
        let ret_typ = returns.to_rust_simplified_type();

        let func = if static_tk.is_some() {
            quote! {
                fn #name<'c>(
                    env: &mut jni::Env<'c>,
                    class: jni::objects::JClass<'c>,
                    #(#args),*
                ) #arrow jni::errors::Result<#ret_typ> #body
            }
        } else {
            quote! {
                fn #name<'c>(
                    env: &mut jni::Env<'c>,
                    this: jni::objects::JObject<'c>,
                    #(#args),*
                ) #arrow jni::errors::Result<#ret_typ> #body
            }
        };

        let arg_types = params
            .iter()
            .map(|t|
                syn::parse_str::<TokenStream>(&t.typ.jtype().unwrap())
                    .map_err(|e| e.to_compile_error())
                    .unwrap_or_else(|t| t))
            .collect::<Vec<_>>();
        let ret = returns.jtype()
            .map(|t| {
                let t = syn::parse_str::<TokenStream>(&t)
                    .map_err(|e| e.to_compile_error())
                    .unwrap_or_else(|t| t);
                quote!(-> #t)
            }).unwrap_or_default();

        nats.append_all(quote! {
            const _: jni::NativeMethod = jni::native_method! {
                java_type = #class_name,
                #static_tk extern #fn_tk #name(#(#arg_types),*)#ret,
            };
            #func
        });
    }
}

impl JavaCodeBlock {
    fn output(&self, uses: &[JavaUse]) -> TokenStream {
        let mut out = TokenStream::new();

        for section in self.sections.iter() {
            match section {
                crate::CodeSection::Tokens(t) => out.append_all(quote!(#t)),
                crate::CodeSection::Block(b) => {
                    let t = b.output(uses);
                    out.append_all(quote!(#t));
                },
                crate::CodeSection::Java(j) => {
                    let t = j.output(uses);
                    out.append_all(quote!(#t));
                },
            }
        }

        match self.delimiter {
            Delimiter::Parenthesis => quote!((#out)),
            Delimiter::Brace => quote!({#out}),
            Delimiter::Bracket => quote!([#out]),
            Delimiter::None => quote!(#out),
        }

    }
}

impl JavaExpression {
    fn output(&self, uses: &[JavaUse]) -> TokenStream {
        match self {
            Self::Expr(e) => e.output(uses),
            Self::StaticFieldWrite(w) => w.output(uses),
            Self::FieldWrite(w) => w.output(uses),
            Self::ArrayWrite(w) => w.output(uses),
            Self::ThrowNew(t) => t.output(uses),
            Self::ThrowObj(t) => t.output(uses),
        }
    }
}

impl JavaExpr {
    fn output(&self, uses: &[JavaUse]) -> TokenStream {
        match self {
            JavaExpr::Error(e) => e.clone(),
            JavaExpr::StaticCall(c) => c.output(uses),
            JavaExpr::MethodCall(c) => c.output(uses),
            JavaExpr::StaticFieldRead(r) => r.output(uses),
            JavaExpr::FieldRead(r) => r.output(uses),
            JavaExpr::ArrayRead(r) => r.output(uses),
            JavaExpr::New(n) => n.output(uses),
            JavaExpr::Alloc(a) => a.output(uses),
        }
    }
}

impl JavaValueExpr {
    fn output(&self, uses: &[JavaUse]) -> TokenStream {
        match self {
            JavaValueExpr::Expr(e) => e.output(uses),
            JavaValueExpr::Block(b) => b.output(uses),
        }
    }
}

impl JavaStaticFieldWrite {
    fn output(&self, uses: &[JavaUse]) -> TokenStream {
        let Self {
            class,
            field,
            signature,
            value,
        } = self;
        let class = class.to_string();
        let field = field.to_string();
        let value = value.output(uses);
        match signature {
            None => {
                syn::Error::new(
                    field.span(),
                    "field needs a signature tag: `Class::field #[\"...\"]`"
                ).to_compile_error()
            }
            Some(JavaSignatureTag(signature)) => {
                quote! {
                    env.set_static_field(
                        jni::jni_str!(#class),
                        jni::jni_str!(#field),
                        jni::jni_sig!(#signature),
                        {#value}.into(),
                    )?
                }
            }
        }
    }
}

impl JavaFieldWrite {
    fn output(&self, uses: &[JavaUse]) -> TokenStream {
        let Self {
            obj,
            field,
            signature,
            value,
        } = self;
        let field = field.to_string();
        let value = value.output(uses);
        match signature {
            None => {
                syn::Error::new(
                    field.span(),
                    "field type could not be determined, please add a tag: `obj.field #[\"...\"]`"
                ).to_compile_error()
            },
            Some(JavaSignatureTag(signature)) => {
                quote! {
                    env.set_field(
                        #obj,
                        jni::jni_str!(#field),
                        jni::jni_sig!(#signature),
                        {#value}.into(),
                    )?
                }
            }
        }
    }
}

impl JavaArrayWrite {
    fn output(&self, uses: &[JavaUse]) -> TokenStream {
        let Self {
            array,
            index,
            signature,
            value,
        } = self;
        let Some(JavaSignatureTag(signature)) = signature else {
            return syn::Error::new(
                array.span(),
                "array element type could not be determined, please add a tag: `array[...] #[\"...\"]`",
            ).to_compile_error()
        };
        let sig = signature.value();
        let index = index.output(uses);
        let value = value.output(uses);
        if sig.starts_with("L") || sig.starts_with("[") {
            // object array
            quote! {
                #array.set_element(
                    env,
                    {#index},
                    {#value}.into(),
                )?
            }
        } else {
            // primitive array
            quote! {
                #array.set_region(
                    env,
                    {#index},
                    &[{#value}.into()],
                )?
            }
        }
    }
}

impl JavaThrowNew {
    fn output(&self, uses: &[JavaUse]) -> TokenStream {
        let Self {
            class,
            message,
        } = self;
        let class = class.to_string();
        let message = message.output(uses);
        quote! {
            return env.throw_new(
                jni::jni_str!(#class),
                jni::strings::JNIString::new({#message}),
            )
        }
    }
}

impl JavaThrowObj {
    fn output(&self, uses: &[JavaUse]) -> TokenStream {
        let value = self.value.output(uses);

        quote! {
            return env.throw(
                {#value}
            )
        }

    }
}

impl JavaStaticCall {
    fn output(&self, uses: &[JavaUse]) -> TokenStream {
        let Self {
            class,
            func,
            args,
            signature,
        } = self;
        let Some(JavaSignatureTag(signature)) = signature else {
            return syn::Error::new(
                func.span(),
                "static method type could not be determined, please add a tag: `Class::func(...) #[\"(...)...\"]`",
            ).to_compile_error()
        };
        let class = class.to_string();
        let func = func.to_string();
        let args = args.iter().map(|a| {
            let a = a.output(uses);
            quote!({#a}.into())
        });
        quote! {
            env.call_static_method(
                jni::jni_str!(#class),
                jni::jni_str!(#func),
                jni::jni_sig!(#signature),
                &[#(#args),*],
            )?
        }
    }
}

impl JavaMethodCall {
    fn output(&self, uses: &[JavaUse]) -> TokenStream {
        let Self {
            obj,
            func,
            args,
            signature,
        } = self;
        let Some(JavaSignatureTag(signature)) = signature else {
            return syn::Error::new(
                func.span(),
                "method type could not be determined, please add a tag: `obj.func(...) #[\"...\"]`",
            ).to_compile_error()
        };
        let func = func.to_string();
        let args = args.iter().map(|a| {
            let a = a.output(uses);
            quote!({#a}.into())
        });
        quote! {
            env.call_method(
                #obj,
                jni::jni_str!(#func),
                jni::jni_sig!(#signature),
                &[#(#args),*],
            )?
        }
    }
}

impl JavaStaticFieldRead {
    fn output(&self, uses: &[JavaUse]) -> TokenStream {
        let Self {
            class,
            field,
            signature,
        } = self;

        let Some(JavaSignatureTag(signature)) = signature else {
            return syn::Error::new(
                field.span(),
                "field type could not be determined, please add a tag: `Class::field #[\"...\"]`",
            ).to_compile_error();
        };
        let class = class.to_string();
        let field = field.to_string();
        let sig = signature.value();
        let sig = match JavaSig::parse(&sig) {
            Ok(s) => s,
            Err(e) => return syn::Error::new(
                signature.span(),
                e,
            ).to_compile_error(),
        };

        let cast = sig.return_cast();

        let base = quote! {
            env.get_static_field(
                jni::jni_str!(#class),
                jni::jni_str!(#field),
                jni::jni_sig!(#signature),
            )? #cast
        };

        // TODO: local_cast
        base
    }
}

impl JavaFieldRead {
    fn output(&self, uses: &[JavaUse]) -> TokenStream {
        let Self {
            obj,
            field,
            signature,
        } = self;
    }
}

impl JavaArrayRead {
    fn output(&self, uses: &[JavaUse]) -> TokenStream {
        let Self {
            array,
            index,
            signature,
        } = self;
    }
}

impl JavaNew {
    fn output(&self, uses: &[JavaUse]) -> TokenStream {
        let Self {
            class,
            args,
            signature,
        } = self;
    }
}

impl JavaAlloc {
    fn output(&self, uses: &[JavaUse]) -> TokenStream {
        let class = self.class.to_string();
    }
}

struct JavaFnSig<'l> {
    args: Vec<JavaSig<'l>>,
    returns: JavaSig<'l>,
}

enum JavaSig<'l> {
    V,
    Z,
    B,
    C,
    S,
    I,
    J,
    F,
    D,
    L(Vec<&'l str>),
    A(Box<Self>),
}

impl<'l> JavaFnSig<'l> {
    fn parse(input: &'l str) -> Result<Self, Cow<'static, str>> {
        let Some((args, ret)) = input.split_once(")") else {
            return Err("no parenthesis found".into())
        };
        let args = args.trim_start_matches("(");
        let mut i = 0;
        let mut arg_types = Vec::new();
        while i < args.len() {
            let next = JavaSig::parse_substr(args, &mut i)?;
            arg_types.push(next);
        }
        let returns = JavaSig::parse(ret)?;
        Ok(Self { args: arg_types, returns })
    }
}

impl<'l> JavaSig<'l> {
    fn parse(input: &'l str) -> Result<Self, Cow<'static, str>> {
        Self::parse_substr(input, &mut 0)
    }
    fn parse_substr(input: &'l str, i: &mut usize) -> Result<Self, Cow<'static, str>> {
        let Some(view) = input.get(*i..) else {
            return Err("EOF".into());
        };
        let Some(c0) = view.get(0..=0) else {
            return Err("EOF".into())
        };
        *i += 1;
        Ok(match c0 {
            "V" => Self::V,
            "Z" => Self::Z,
            "B" => Self::B,
            "C" => Self::C,
            "S" => Self::S,
            "I" => Self::I,
            "J" => Self::J,
            "F" => Self::F,
            "D" => Self::D,
            "[" => {
                let mut i2 = 0;
                let t = Self::parse_substr(&view[1..], &mut i2)?;
                *i += i2;
                Self::A(Box::new(t))
            },
            "L" => {
                let Some((class_name, _)) = view[1..].split_once(";") else {
                    return Err(format!("Expected ';' after class path: {}", &view[1..]).into())
                };
                *i += class_name.len() + 1;
                let parts = class_name.split("/").collect();
                Self::L(parts)
            },
            _ => return Err(format!("Unrecognized type marker: {c0}").into())
        })
    }
    fn return_cast(&self) -> TokenStream {
        match self {
            JavaSig::V => quote!(),
            JavaSig::Z => quote!(.z()?),
            JavaSig::B => quote!(.b()?),
            JavaSig::C => quote!(.c()?),
            JavaSig::S => quote!(.s()?),
            JavaSig::I => quote!(.i()?),
            JavaSig::J => quote!(.j()?),
            JavaSig::F => quote!(.f()?),
            JavaSig::D => quote!(.d()?),
            JavaSig::L(_l) => {
                // TODO: lookup local_cast type from uses
                quote!(.l()?)
            },
            JavaSig::A(_a) => {
                // TODO: array type lookup
                quote!(.l()?)
            },
        }
    }
}



impl JavaFnSignature {
    fn jni_signature(&self) -> String {
        let r = self.returns.java_signature();
        let p = self.params
            .iter()
            .map(|p| p.typ.java_signature())
            .collect::<Vec<_>>()
            .join("");
        format!("({p}){r}")
    }
}

impl JavaParam {
    fn rust_to_java(&self) -> TokenStream {
        let name = &self.name;
        quote! {
            let #name = java_jni_extras::JavaTyped::into_java(#name, env)?;
        }
    }
    fn to_rust_arg(&self, uses: &[JavaUse]) -> TokenStream {
        let name = &self.name;
        let typ: Type = self.typ.to_rust_type(uses);

        quote! {
            #name: #typ
        }
    }
}

impl JavaType {
    fn is_primitive(&self) -> bool {
        !matches!(self, Self::String | Self::Array(_) | Self::Object(_))
    }
    fn is_custom_object(&self) -> bool {
        if let Self::Object(o) = self {
            let n = o.to_string();
            n != "java.lang.Object"
        } else {
            false
        }
    }
    fn jni_conversion(&self, uses: &[JavaUse]) -> TokenStream {
        match self {
            JavaType::Void => quote!(Ok(())),
            JavaType::Boolean => quote!(ret.z()),
            JavaType::Byte => quote!(ret.b()),
            JavaType::Char => quote!(ret.c()),
            JavaType::Short => quote!(ret.s()),
            JavaType::Int => quote!(ret.i()),
            JavaType::Long => quote!(ret.j()),
            JavaType::Float => quote!(ret.f()),
            JavaType::Double => quote!(ret.d()),
            JavaType::String => quote! {
                jni::objects::JString::cast_local(env, ret.l()?)
            },
            JavaType::Object(o) => {
                if o.to_string() == "java.lang.Object" {
                    quote!(ret.l())
                } else {
                    let n = o.0.last().unwrap();
                    if let Some(mut rt) = uses
                        .iter()
                        .find_map(|u| if u.java_path == *o {
                            Some(u.rust_path.clone())
                        } else { None })
                    {
                        *rt.parts.last_mut().unwrap() = n.clone();
                        quote!{
                            Ok(#rt(ret.l()?))
                        }
                    } else {
                        quote!(ret.l())
                    }
                }
            },
            array @ JavaType::Array(_) => {
                let t = array.to_rust_concrete_type(uses);
                quote! {
                    env.cast_local::<#t>(ret.l()?)
                }
            },
        }
    }

    fn to_rust_return_type(&self, uses: &[JavaUse]) -> Type {
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
                Self::resolve_rust_type(o, uses)
            }
            JavaType::Array(a) => {
                let at = a.to_rust_concrete_type(uses);
                if a.is_primitive() {
                    parse_quote!(jni::objects::JPrimitiveArray<'c, #at>)
                } else {
                    parse_quote!(jni::objects::JObjectArray<'c, #at>)
                }
            }
        }
    }
    fn to_rust_type(&self, uses: &[JavaUse]) -> Type {
        match self {
            JavaType::Void => parse_quote!(()),
            JavaType::Boolean => parse_quote!(impl java_jni_extras::JavaTyped<'c, JType=jni::sys::jboolean>),
            JavaType::Byte => parse_quote!(impl java_jni_extras::JavaTyped<'c, JType=jni::sys::jbyte>),
            JavaType::Char => parse_quote!(impl java_jni_extras::JavaTyped<'c, JType=jni::sys::jchar>),
            JavaType::Short => parse_quote!(impl java_jni_extras::JavaTyped<'c, JType=jni::sys::jshort>),
            JavaType::Int => parse_quote!(impl java_jni_extras::JavaTyped<'c, JType=jni::sys::jint>),
            JavaType::Long => parse_quote!(impl java_jni_extras::JavaTyped<'c, JType=jni::sys::jlong>),
            JavaType::Float => parse_quote!(impl java_jni_extras::JavaTyped<'c, JType=jni::sys::jfloat>),
            JavaType::Double => parse_quote!(impl java_jni_extras::JavaTyped<'c, JType=jni::sys::jdouble>),
            JavaType::String => parse_quote!(impl java_jni_extras::JavaTyped<'c, JType=jni::objects::JString<'c>>),
            JavaType::Object(o) => {
                Self::resolve_rust_type(o, uses)
            }
            JavaType::Array(a) => {
                let at = a.to_rust_concrete_type(uses);
                if a.is_primitive() {
                    parse_quote!(impl java_jni_extras::JavaTyped<'c, JType=JPrimitiveArray<'c, #at>>)
                } else {
                    parse_quote!(impl java_jni_extras::JavaTyped<'c, JType=JObjectArray<'c, #at>>)
                }
            }
        }
    }
    fn to_rust_concrete_type(&self, uses: &[JavaUse]) -> Type {
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
                Self::resolve_rust_type(o, uses)
            }
            JavaType::Array(a) => {
                let at = a.to_rust_concrete_type(uses);
                if a.is_primitive() {
                    parse_quote!(jni::objects::JPrimitiveArray<'c, #at>)
                } else {
                    parse_quote!(jni::objects::JObjectArray<'c, #at>)
                }
            }
        }
    }
    fn to_rust_simplified_type(&self) -> Type {
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
            JavaType::Object(_) => {
                parse_quote!(jni::objects::JObject<'c>)
            }
            JavaType::Array(a) => {
                let at = a.to_rust_simplified_type();
                if a.is_primitive() {
                    parse_quote!(jni::objects::JPrimitiveArray<'c, #at>)
                } else {
                    parse_quote!(jni::objects::JObjectArray<'c, #at>)
                }
            }
        }
    }

    fn resolve_rust_type(o: &DottedName, uses: &[JavaUse]) -> Type {
        if o.to_string() == "java.lang.Object" {
            parse_quote!(jni::objects::JObject<'c>)
        } else {
            let n = o.0.last().unwrap();
            if let Some(mut rt) = uses
                .iter()
                .find_map(|u| if u.java_path == *o {
                    Some(u.rust_path.clone())
                } else {None}) {
                *rt.parts.last_mut().unwrap() = n.clone();
                parse_quote!(#rt<'c>)
            } else {
                parse_quote!(jni::objects::JObject<'c>)
            }
        }
    }

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

impl DottedName {
    fn class_path(&self) -> String {
        format!("L{};", self.0.iter().map(ToString::to_string).collect::<Vec<_>>().join("/"))
    }
}

impl ToTokens for PathSepName {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let parts = &self.parts;
        tokens.append_all(quote!{
            #(#parts)::*
        });
    }
}


