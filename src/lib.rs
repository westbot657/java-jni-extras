use jni::Env;
use jni::objects::{JObject, JObjectArray, JPrimitiveArray, JString, Reference, TypeArray};
use jni::sys::{jboolean, jbyte, jchar, jdouble, jfloat, jint, jlong, jshort};
pub use java_jni_extras_proc::java_class;

pub trait JavaTyped<'c> {
    type JType;
    fn into_java<'local>(self, env: &'local mut Env<'c>) -> jni::errors::Result<Self::JType>;
}

trait JavaObject {}
impl<'c> JavaObject for JObject<'c> {}
impl<'c> JavaObject for JObjectArray<'c> {}
impl<'c, T: TypeArray> JavaObject for JPrimitiveArray<'c, T> {}

pub trait JavaPrimitive: Sized {
    type P: Sized + TypeArray;
    fn as_java(&self) -> Self::P;
}
macro_rules! java_primitive {
    ( $T:ty, $j:ty ) => {
        const _: () = const {
            assert!(std::mem::size_of::<$T>() == std::mem::size_of::<<$T as JavaPrimitive>::P>());
        };
        impl JavaPrimitive for $T {
            type P = $j;
            fn as_java(&self) -> Self::P {
                *self
            }
        }
        impl<'c, 'r> JavaTyped<'c> for &[$T]
        {
            type JType = JPrimitiveArray<'c, <$T as JavaPrimitive>::P>;
            fn into_java<'local>(self, env: &'local mut Env<'c>) -> jni::errors::Result<Self::JType> {
                let arr = JPrimitiveArray::<<$T as JavaPrimitive>::P>::new(env, self.len())?;
                arr.set_region(env, 0, unsafe { std::mem::transmute(self) })?;
                Ok(arr)
            }
        }
        impl<'c, 'r, const N: usize> JavaTyped<'c> for &[$T; N]
        {
            type JType = JPrimitiveArray<'c, <$T as JavaPrimitive>::P>;
            fn into_java<'local>(self, env: &'local mut Env<'c>) -> jni::errors::Result<Self::JType> {
                let arr = JPrimitiveArray::<<$T as JavaPrimitive>::P>::new(env, self.len())?;
                arr.set_region(env, 0, unsafe { std::mem::transmute(self.as_slice()) })?;
                Ok(arr)
            }
        }
        impl<'c, 'r> JavaTyped<'c> for Vec<$T>
        {
            type JType = JPrimitiveArray<'c, <$T as JavaPrimitive>::P>;
            fn into_java<'local>(self, env: &'local mut Env<'c>) -> jni::errors::Result<Self::JType> {
                let arr = JPrimitiveArray::<<$T as JavaPrimitive>::P>::new(env, self.len())?;
                arr.set_region(env, 0, unsafe { std::mem::transmute(self.as_slice()) })?;
                Ok(arr)
            }
        }
    };
}

java_primitive! { i8, jbyte }
java_primitive! { u16, jchar }
java_primitive! { i16, jshort }
java_primitive! { i32, jint }
java_primitive! { i64, jlong }
java_primitive! { f32, jfloat }
java_primitive! { f64, jdouble }
java_primitive! { bool, jboolean }

impl<'c, T: Sized + JavaPrimitive> JavaTyped<'c> for T {
    type JType = T::P;
    fn into_java<'local>(self, _env: &'local mut Env<'c>) -> jni::errors::Result<Self::JType> {
        Ok(self.as_java())
    }
}

impl<'c, 'r> JavaTyped<'c> for &str {
    type JType = JString<'c>;
    fn into_java<'local>(self, env: &'local mut Env<'c>) -> jni::errors::Result<Self::JType> {
        env.new_string(self)
    }
}

impl<'c, 'r> JavaTyped<'c> for String {
    type JType = JString<'c>;
    fn into_java<'local>(self, env: &'local mut Env<'c>) -> jni::errors::Result<Self::JType> {
        env.new_string(self)
    }
}

impl<'c, 'r, T> JavaTyped<'c> for &[T]
where
    T: JavaObject + JavaTyped<'c> + Reference<Kind<'c> = T> + AsRef<JObject<'c>>,
    T::JType: 'c + Reference<Kind<'c> = T::JType> + AsRef<T::JType>,
{
    type JType = JObjectArray<'c, T::JType>;
    fn into_java<'local>(self, env: &'local mut Env<'c>) -> jni::errors::Result<Self::JType> {
        let null = T::JType::null();
        let arr: Self::JType = JObjectArray::<'c, T::JType>::new(env, self.len(), null)?;
        for (i, v) in self.iter().enumerate() {
            let v = env.new_local_ref(v)?;
            let v = JavaTyped::<'c>::into_java(v, env)?;
            arr.set_element(env, i, v)?;
        }
        Ok(arr)
    }
}

impl<'c, 'r, T, const N: usize> JavaTyped<'c> for &[T; N]
where
    T: JavaObject + JavaTyped<'c> + Reference<Kind<'c> = T> + AsRef<JObject<'c>>,
    T::JType: 'c + Reference<Kind<'c> = T::JType> + AsRef<T::JType>,
{
    type JType = JObjectArray<'c, T::JType>;
    fn into_java<'local>(self, env: &'local mut Env<'c>) -> jni::errors::Result<Self::JType> {
        let null = T::JType::null();
        let arr: Self::JType = JObjectArray::<'c, T::JType>::new(env, self.len(), null)?;
        for (i, v) in self.iter().enumerate() {
            let v = env.new_local_ref(v)?;
            let v = JavaTyped::<'c>::into_java(v, env)?;
            arr.set_element(env, i, v)?;
        }
        Ok(arr)
    }
}

impl<'c, 'r, T> JavaTyped<'c> for Vec<T>
where
    T: JavaObject + JavaTyped<'c> + Reference<Kind<'c> = T> + AsRef<JObject<'c>>,
    T::JType: 'c + Reference<Kind<'c> = T::JType> + AsRef<T::JType>,
{
    type JType = JObjectArray<'c, T::JType>;
    fn into_java<'local>(self, env: &'local mut Env<'c>) -> jni::errors::Result<Self::JType> {
        let null = T::JType::null();
        let arr: Self::JType = JObjectArray::<'c, T::JType>::new(env, self.len(), null)?;
        for (i, v) in self.into_iter().enumerate() {
            let v = JavaTyped::<'c>::into_java(v, env)?;
            arr.set_element(env, i, v)?;
        }
        Ok(arr)
    }
}

impl<'c, 'r> JavaTyped<'c> for &[&str] {
    type JType = JObjectArray<'c, JString<'c>>;
    fn into_java<'local>(self, env: &'local mut Env<'c>) -> jni::errors::Result<Self::JType> {
        let null = JString::null();
        let arr: Self::JType = JObjectArray::<'c, JString>::new(env, self.len(), null)?;
        for (i, v) in self.iter().enumerate() {
            let v = v.into_java(env)?;
            arr.set_element(env, i, v)?;
        }
        Ok(arr)
    }
}

impl<'c, 'r, const N: usize> JavaTyped<'c> for &[&str; N] {
    type JType = JObjectArray<'c, JString<'c>>;
    fn into_java<'local>(self, env: &'local mut Env<'c>) -> jni::errors::Result<Self::JType> {
        let null = JString::null();
        let arr: Self::JType = JObjectArray::<'c, JString>::new(env, self.len(), null)?;
        for (i, v) in self.iter().enumerate() {
            let v = v.into_java(env)?;
            arr.set_element(env, i, v)?;
        }
        Ok(arr)
    }
}

impl<'c, 'r> JavaTyped<'c> for Vec<&str> {
    type JType = JObjectArray<'c, JString<'c>>;
    fn into_java<'local>(self, env: &'local mut Env<'c>) -> jni::errors::Result<Self::JType> {
        let null = JString::null();
        let arr: Self::JType = JObjectArray::<'c, JString>::new(env, self.len(), null)?;
        for (i, v) in self.into_iter().enumerate() {
            let v = v.into_java(env)?;
            arr.set_element(env, i, v)?;
        }
        Ok(arr)
    }
}

impl<'c, 'r> JavaTyped<'c> for &[String] {
    type JType = JObjectArray<'c, JString<'c>>;
    fn into_java<'local>(self, env: &'local mut Env<'c>) -> jni::errors::Result<Self::JType> {
        let null = JString::null();
        let arr: Self::JType = JObjectArray::<'c, JString>::new(env, self.len(), null)?;
        for (i, v) in self.iter().enumerate() {
            let v = v.clone().into_java(env)?;
            arr.set_element(env, i, v)?;
        }
        Ok(arr)
    }
}

impl<'c, 'r, const N: usize> JavaTyped<'c> for &[String; N] {
    type JType = JObjectArray<'c, JString<'c>>;
    fn into_java<'local>(self, env: &'local mut Env<'c>) -> jni::errors::Result<Self::JType> {
        let null = JString::null();
        let arr: Self::JType = JObjectArray::<'c, JString>::new(env, self.len(), null)?;
        for (i, v) in self.iter().enumerate() {
            let v = v.clone().into_java(env)?;
            arr.set_element(env, i, v)?;
        }
        Ok(arr)
    }
}

impl<'c, 'r> JavaTyped<'c> for Vec<String> {
    type JType = JObjectArray<'c, JString<'c>>;
    fn into_java<'local>(self, env: &'local mut Env<'c>) -> jni::errors::Result<Self::JType> {
        let null = JString::null();
        let arr: Self::JType = JObjectArray::<'c, JString>::new(env, self.len(), null)?;
        for (i, v) in self.into_iter().enumerate() {
            let v = v.into_java(env)?;
            arr.set_element(env, i, v)?;
        }
        Ok(arr)
    }
}

impl<'c> JavaTyped<'c> for JObject<'c> {
    type JType = Self;
    fn into_java<'local>(self, _env: &'local mut Env<'c>) -> jni::errors::Result<Self::JType> {
        Ok(self)
    }
}

impl<'c> JavaTyped<'c> for JObjectArray<'c> {
    type JType = Self;
    fn into_java<'local>(self, _env: &'local mut Env<'c>) -> jni::errors::Result<Self::JType> {
        Ok(self)
    }
}

impl<'c, T: TypeArray> JavaTyped<'c> for JPrimitiveArray<'c, T> {
    type JType = Self;
    fn into_java<'local>(self, _env: &'local mut Env<'c>) -> jni::errors::Result<Self::JType> {
        Ok(self)
    }
}

pub enum MaybeJavaTyped<'c, T>
where
    T: JavaTyped<'c>
{
    Java(T::JType),
    Rust(T),
}

impl<'c, T> JavaTyped<'c> for MaybeJavaTyped<'c, T>
where
    T: JavaTyped<'c>
{
    type JType = T::JType;
    fn into_java<'local>(self, env: &'local mut Env<'c>) -> jni::errors::Result<Self::JType> {
        match self {
            Self::Rust(r) => r.into_java(env),
            Self::Java(j) => Ok(j)
        }
    }
}
