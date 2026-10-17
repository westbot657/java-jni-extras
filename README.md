# java-jni-extras

Defines a proc macro: `java_class_decl`
which lets you define jni exports and wrapper functions in a java syntax.

Example:
```rs

java_class! {
    package com.example;

    import org.joml.Vector3f;
    import org.joml.Vector4f;

    use crate::com.example.OtherClass;

    class ExampleClass {
        value: float;
        static next: int;

        static native fn createNew(value: float) -> ExampleClass {
            let ec = #alloc ExampleClass();
            #ec.value = value;
            ec
        }

        native fn add(other: float) {
            let value = #this.value;
            value += other;
            #this.value = value;
        }
    }
}

```

Todo:
-[ ] Type inference engine.
    -[ ] Level 1: track local variable types where possible.
    -[ ] Level 2: lookup external class types. (org.joml.* for example)
    -[ ] Level 3: lookup rust macro declarations for type annotations.
-[ ] Fix mutability issue in the scenario of: #this.x = #this.y (env is mutably borrowed twice)
-[ ] import-aware signature tags / more ergonimic syntax.
-[ ] fix all compile errors to report positions and not make the macro error.
-[ ] allow 'Self' as class type in function signatures, new, and alloc.




