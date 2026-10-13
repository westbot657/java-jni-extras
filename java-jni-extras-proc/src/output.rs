use quote::{ToTokens, TokenStreamExt, quote};

use crate::process::JniClass;

impl ToTokens for JniClass {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let Self {
            uses,
            struct_name,
            class_name,
            classpath,
            package,
            fields,
            bindings,
            natives,
            errors,
        } = self;

        

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


