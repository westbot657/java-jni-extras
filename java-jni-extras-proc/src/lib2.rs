use syn::Ident;

use crate::parse::DottedName;

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

//#[proc_macro]
pub fn java_class2(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    input
}

















