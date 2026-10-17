use std::fmt::Display;

use proc_macro2::{Delimiter, TokenStream};
use quote::quote;
use syn::punctuated::Punctuated;
use syn::{Ident, LitStr, Token, parse_macro_input};

pub(crate) mod parse;
pub(crate) mod process;
pub(crate) mod output;

pub(crate) mod kw {
    use syn::custom_keyword;

    custom_keyword!(void);
    custom_keyword!(boolean);
    custom_keyword!(byte);
    custom_keyword!(char);
    custom_keyword!(short);
    custom_keyword!(int);
    custom_keyword!(long);
    custom_keyword!(float);
    custom_keyword!(double);
    custom_keyword!(native);
    custom_keyword!(throw);
    custom_keyword!(new);
    custom_keyword!(import);
    custom_keyword!(package);
    custom_keyword!(class);

    custom_keyword!(alloc);
}

#[derive(PartialEq)]
pub(crate) enum JavaType {
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

#[derive(Clone)]
pub(crate) struct DottedName(Punctuated<Ident, Token![.]>);

impl PartialEq for DottedName {
    fn eq(&self, other: &Self) -> bool {
        self.0.len() == other.0.len() && (
            self.0
                .iter()
                .zip(other.0.iter())
                .all(|(a, b)| a == b)
        )
    }
}

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
    pub(crate) package: DottedName,
    pub(crate) imports: Vec<JavaImport>,
    pub(crate) uses: Vec<JavaUse>,
    pub(crate) class: JavaClass,
    pub(crate) errors: Vec<TokenStream>,
}

pub(crate) struct JavaImport {
    pub(crate) item: DottedName,
}

pub(crate) struct JavaUse {
    pub(crate) rust_path: PathSepName,
    pub(crate) java_path: DottedName,
}

#[derive(Default, Clone, Copy)]
pub(crate) enum PathSepPrefix {
    #[default]
    None,
    This,
    Super,
    Crate,
}

#[derive(Clone)]
pub(crate) struct PathSepName {
    pub(crate) prefix: PathSepPrefix,
    pub(crate) parts: Vec<Ident>,
}

pub(crate) struct JavaClass {
    pub(crate) name: Ident,
    pub(crate) body: ClassBody,
}

pub(crate) struct ClassBody {
    pub(crate) fields: Vec<JavaField>,
    pub(crate) bindings: Vec<JavaBinding>,
    pub(crate) natives: Vec<NativeFunction>,
    pub(crate) errors: Vec<TokenStream>,
}

pub(crate) struct JavaField {
    pub(crate) static_tk: Option<Token![static]>,
    pub(crate) name: Ident,
    pub(crate) typ: JavaType,
}

pub(crate) struct JavaBinding {
    pub(crate) static_tk: Option<Token![static]>,
    pub(crate) signature: JavaFnSignature,
}

pub(crate) struct NativeFunction {
    pub(crate) static_tk: Option<Token![static]>,
    pub(crate) signature: JavaFnSignature,
    pub(crate) body: JavaCodeBlock,
}

pub(crate) struct JavaFnSignature {
    pub(crate) fn_tk: Token![fn],
    pub(crate) name: Ident,
    pub(crate) params: Punctuated<JavaParam, Token![,]>,
    pub(crate) arrow: Option<Token![->]>,
    pub(crate) returns: JavaType,
}

pub(crate) struct JavaParam {
    pub(crate) name: Ident,
    pub(crate) typ: JavaType,
}

pub(crate) struct JavaCodeBlock {
    pub(crate) delimiter: Delimiter,
    pub(crate) sections: Vec<CodeSection>,
}

pub(crate) enum CodeSection {
    Tokens(TokenStream),
    Block(JavaCodeBlock),
    Java(JavaExpression),
}

/// Matches:
///  <expr>
///  #Class::field #["..."] = ...;
///  #obj.field #["..."] = ...;
///  #arr[...] #["..."] = ...;
///  #throw new Error("...");
///  #throw ...;
pub(crate) enum JavaExpression {
    Expr(JavaExpr),
    StaticFieldWrite(JavaStaticFieldWrite),
    FieldWrite(JavaFieldWrite),
    ArrayWrite(JavaArrayWrite),
    ThrowNew(JavaThrowNew),
    ThrowObj(JavaThrowObj),
}

/// Matches:
///  #Class::func(...) #["..."]
///  #obj.method(...) #["..."]
///  #Class::field #["..."]
///  #obj.field #["..."]
///  #arr[...] #["..."]
///  #new Class(...) #["..."]
///  #alloc Class()
///  (error) #...;
pub(crate) enum JavaExpr {
    Error(TokenStream),
    StaticCall(JavaStaticCall),
    MethodCall(JavaMethodCall),
    StaticFieldRead(JavaStaticFieldRead),
    FieldRead(JavaFieldRead),
    ArrayRead(Box<JavaArrayRead>),
    New(JavaNew),
    Alloc(JavaAlloc),
}

/// Matches:
///  <expr>
///  or
///  {...}
///  or
///  <single block-expression>
pub(crate) enum JavaValueExpr {
    Expr(JavaExpr),
    Block(JavaCodeBlock),
}

/// matches function and field signatures:
/// #["V(Ljava/lang/String;)"]
/// #["Z"]
pub(crate) struct JavaSignatureTag(LitStr);

/// #Unknown::func(...) #["V(Ljava/lang/String;IIF)"]
pub(crate) struct JavaStaticCall {
    pub(crate) class: DottedName,
    pub(crate) func: Ident,
    pub(crate) args: Punctuated<JavaValueExpr, Token![,]>,
    /// May be None if the function has no args and the return type can be inferred or is void.
    pub(crate) signature: Option<JavaSignatureTag>,
}

/// #unknown.method(...) #["V(Ljava/lang/String;FF)"]
pub(crate) struct JavaMethodCall {
    pub(crate) obj: Ident,
    pub(crate) func: Ident,
    pub(crate) args: Punctuated<JavaValueExpr, Token![,]>,
    /// May be None if the function has no args and the return type can be inferred or is void.
    pub(crate) signature: Option<JavaSignatureTag>,
}

/// #Class::x #["J"]
pub(crate) struct JavaStaticFieldRead {
    pub(crate) class: DottedName,
    pub(crate) field: Ident,
    /// only needed if the type can't be inferred.
    pub(crate) signature: Option<JavaSignatureTag>,
}

/// #Class::x #["Ljava/lang/String;"] = ...;
pub(crate) struct JavaStaticFieldWrite {
    pub(crate) class: DottedName,
    pub(crate) field: Ident,
    /// only needed if the type can't be inferred.
    pub(crate) signature: Option<JavaSignatureTag>,
    pub(crate) value: JavaValueExpr,
}

/// #this.x #["Ljava/lang/String;"] = ...;
pub(crate) struct JavaFieldWrite {
    pub(crate) obj: Ident,
    pub(crate) field: Ident,
    /// only needed if the type can't be inferred.
    pub(crate) signature: Option<JavaSignatureTag>,
    pub(crate) value: JavaValueExpr,
}

/// #this.x #["J"]
pub(crate) struct JavaFieldRead {
    pub(crate) obj: Ident,
    pub(crate) field: Ident,
    /// only needed if the type can't be inferred.
    pub(crate) signature: Option<JavaSignatureTag>,
}

/// #arr[...] #["Z"] = ...;
pub(crate) struct JavaArrayWrite {
    pub(crate) array: Ident,
    pub(crate) index: JavaValueExpr,
    /// only needed if the type can't be inferred.
    pub(crate) signature: Option<JavaSignatureTag>,
    pub(crate) value: JavaValueExpr,
}

/// #arr[...] #["I"]
pub(crate) struct JavaArrayRead {
    pub(crate) array: Ident,
    pub(crate) index: JavaValueExpr,
    /// only needed if the type can't be inferred.
    pub(crate) signature: Option<JavaSignatureTag>,
}

/// #throw new Class(...);
pub(crate) struct JavaThrowNew {
    pub(crate) class: DottedName,
    pub(crate) message: JavaValueExpr,
}

/// #throw ...;
pub(crate) struct JavaThrowObj {
    pub(crate) value: JavaValueExpr,
}

/// #new Class(...) #["IJJLcom/beatcraft/interop/Note;"]
pub(crate) struct JavaNew {
    pub(crate) class: DottedName,
    pub(crate) args: Punctuated<JavaValueExpr, Token![,]>,
    /// May be None if the ctor has no args or can all be inferred.
    pub(crate) signature: Option<JavaSignatureTag>,
}

/// #alloc Class()
pub(crate) struct JavaAlloc {
    pub(crate) class: DottedName,
}

/*
 * java_class! {
 *     package com.beatcraft.interop;
 *
 *     import org.joml.Vector3f;
 *
 *     use crate::com.beatcraft.interop.Info;
 *
 *     class BeatcraftCore {
 *         handle: long;
 *         static thing: int;
 *
 *         static native fn func(x: float, y: float) -> BeatcraftCore {
 *             let handle = ...
 *             let this = #alloc BeatcraftCore();
 *             #this.handle = handle;
 *             this
 *         }
 *         native fn method(x: float, y: float) -> float {
 *             let handle = #this.handle;
 *         }
 *         static fn thing(...) -> ...;
 *         fn thing2(...) -> ...;
 *     }
 *
 * }
 *
 */

/// Allows writing a java class binding with more ergonomic syntax,
/// and simplifies many requirements such as class paths, and jni plumbing
///
/// Example usage:
/// ```rs
/// java_class! {
///     package com.example.project;
///
///     // You can import classes to avoid writting fully qualified
///     // class names later on.
///     import org.joml.Vector3f;
///
///     // You can also use classes defined elsewhere.
///     // This allows for functions to return the wrapper struct,
///     // instead of just JObject.
///     // the java classpath is also required for the same reason
///     // that imports are allowed.
///     use crate::class::com.example.project.Class2;
///
///     class MyClass {
///         // class fields can be specified to simplify later code.
///         // note: this is currently experimental, and field types here
///         // apply to any java field access from any class.
///         static val1: Vector3f;
///         handle: long;
///         val2: int;
///
///         // prefixing `native` before a function means the function
///         // implementation is here in rust.
///         // prefixing `static` makes this a static method.
///         // static native functions always have implicit access to
///         // 2 extra variables:
///         // env: &mut Env<'c>
///         // class: &JClass<'c>
///         static native fn new_1(a: float, b: float) -> MyClass {
///             let handle = ...;
///             let this = #alloc MyClass();
///
///             // the {...} around handle are needed since
///             // this allows for more complex rust code to be used,
///             // such as function calls.
///             #this.handle = {handle};
///             #this.val2 = {0};
///
///             this
///         }
///
///         // non-static native functions have implicit access to:
///         // env: &mut Env<'c>
///         // this: &JObject<'c>
///         native fn increment() {
///             let mut val2 = #this.val2;
///             val2 += 1;
///             #this.val2 = {val2};
///         }
///
///         // non-native functions allow calling java functions directly in rust.
///         // note that overloaded functions are impossible to define more than 1 of.
///         static fn do_java_stuff(i: int) -> String;
///         fn do_stuff2() -> int;
///     }
/// }
/// ```
#[proc_macro]
pub fn java_class(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let class: JavaClassDef = parse_macro_input!(input);
    let class = class.resolve();
    quote! {
        #class
    }.into()
}

















