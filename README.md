# java-jni-extras

Defines a proc macro: `java_class_decl`
which lets you define jni exports and wrapper functions in a java syntax.

Example:
```rs

java_class_decl! {
    package com.example.project;
    
    class MyClass {
        native int nativeAdd(int, int);
        
        String getName();
    }
}

pub fn nativeAdd<'caller>(
    env: &mut Env<'caller>,
    this: JObject<'caller>,
    a: jint,
    b: jint,
) -> Result<jint, jni::errors::Error> {
    MyClass::_validate_interface(env)?; // Checks if all declared methods exist for the java-defined type
    
    let s: String = MyClass::getName(env, &this)?;
    println!("instance name: {}", s);
    
    Ok(a + b)
}

```