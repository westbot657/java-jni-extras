
use std::borrow::Cow;

use proc_macro2::TokenStream;
use syn::{Ident, parse_quote};

use crate::{ClassBody, DottedName, JavaBinding, JavaClass, JavaClassDef, JavaCodeBlock, JavaExpr, JavaExpression, JavaField, JavaFnSignature, JavaImport, JavaSignatureTag, JavaType, JavaUse, JavaValueExpr, NativeFunction};

pub struct JniClass {
    pub struct_name: Ident,

    pub class_name: String,
    pub classpath: String,
    pub package: String,

    pub fields: Vec<JavaField>,
    pub bindings: Vec<JavaBinding>,
    pub natives: Vec<NativeFunction>,

    pub errors: Vec<TokenStream>,
}

impl DottedName {
    fn classpath(&self, classname: &str) -> String {
        format!(
            "L{}/{};",
            self.0
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("/"),
            classname
        )
    }
}

impl JavaClassDef {
    /// Resolves imports/uses and signature tags
    pub fn resolve(self) -> JniClass {
        let Self { package, mut imports, uses, class, mut errors } = self;
        let JavaClass { name, body } = class;
        let ClassBody { mut fields, mut bindings, mut natives, errors: mut e2 } = body;
        errors.append(&mut e2);

        let package_name = package.to_string();
        let n = name.to_string();
        let class_name = format!("{}.{}", package_name, n);
        let classpath = package.classpath(&n);

        imports.insert(0, JavaImport {
            item: syn::parse_str(&class_name).unwrap()
        });
        imports.insert(0, JavaImport {
            item: parse_quote!(java.lang.String)
        });
        imports.insert(0, JavaImport {
            item: parse_quote!(java.lang.Object)
        });

        for field in fields.iter_mut() {
            if let Err(e) = field.typ.resolve(&imports, &uses) {
                errors.push(parse_quote! {
                    compile_error!(#e);
                });
            }
        }
        for binding in bindings.iter_mut() {
            if let Err(e) = binding.signature.resolve(&imports, &uses) {
                errors.push(parse_quote! {
                    compile_error!(#e);
                })
            }
        }

        for native in natives.iter_mut() {
            if let Err(e) = native.signature.resolve(&imports, &uses) {
                errors.push(parse_quote! {
                    compile_error!(#e);
                });
            }
            if let Err(e) = native.body.resolve(&imports, &uses, &fields, ) {
                errors.push(parse_quote! {
                    compile_error!(#e);
                });
            }
        }

        JniClass {
            struct_name: name,
            class_name,
            classpath,
            package: package_name,
            fields,
            bindings,
            natives,
            errors,
        }
    }
}

impl DottedName {
    fn is_dotted(&self) -> bool {
        self.0.len() > 1
    }
    fn with_name_token(&self, name: Ident) -> Self {
        let mut s = self.clone();
        let _ = s.0.pop();
        s.0.push(name);
        s
    }
    fn name(&self) -> &Ident {
        self.0.last().unwrap()
    }
}

impl JavaCodeBlock {
    fn resolve(&mut self, imports: &[JavaImport], uses: &[JavaUse], fields: &[JavaField]) -> Result<(), String> {
        let mut failures = Vec::new();
        for section in self.sections.iter_mut() {
            if let Err(e) = match section {
                crate::CodeSection::Tokens(_) => continue,
                crate::CodeSection::Block(block) => block.resolve(imports, uses, fields),
                crate::CodeSection::Java(expr) => expr.resolve(imports, uses, fields),
            } {
                failures.push(e);
            }
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(failures.join("; "))
        }
    }
}

impl JavaExpression {
    fn resolve(&mut self, imports: &[JavaImport], uses: &[JavaUse], fields: &[JavaField]) -> Result<(), String> {
        match self {
            JavaExpression::Expr(expr) => expr.resolve(imports, uses, fields),
            JavaExpression::StaticFieldWrite(w) => {
                let mut errs = Vec::with_capacity(2);
                if let Err(e) = w.class.resolve(imports, uses) {
                    errs.push(e);
                }
                if let Err(e) = w.value.resolve(imports, uses, fields) {
                    errs.push(e);
                }
                if let Err(e) = resolve_static_field_signature(&w.class, &w.field, &mut w.signature, fields) {
                    errs.push(e);
                }
                if errs.is_empty() {
                    Ok(())
                } else {
                    Err(errs.join("; "))
                }
            },
            JavaExpression::FieldWrite(w) => {
                let mut errs = Vec::new();
                if let Err(e) = resolve_field_signature(&w.obj, &w.field, &mut w.signature, fields) {
                    errs.push(e);
                }
                if let Err(e) = w.value.resolve(imports, uses, fields) {
                    errs.push(e);
                }
                if errs.is_empty() {
                    Ok(())
                } else {
                    Err(errs.join("; "))
                }
            },
            JavaExpression::ArrayWrite(w) => {
                let mut errs = Vec::with_capacity(2);
                if let Err(e) = w.index.resolve(imports, uses, fields) {
                    errs.push(e);
                }
                if let Err(e) = resolve_array_signature(&w.array, &mut w.signature) {
                    errs.push(e);
                }
                if let Err(e) = w.value.resolve(imports, uses, fields) {
                    errs.push(e);
                }
                if errs.is_empty() {
                    Ok(())
                } else {
                    Err(errs.join("; "))
                }
            },
            JavaExpression::ThrowNew(t) => {
                t.class.resolve(imports, uses)
            },
            JavaExpression::ThrowObj(t) => {
                t.value.resolve(imports, uses, fields)
            },
        }
    }
}

impl JavaValueExpr {
    fn resolve(&mut self, imports: &[JavaImport], uses: &[JavaUse], fields: &[JavaField]) -> Result<(), String> {
        todo!()
    }
}

impl JavaExpr {
    fn resolve(&mut self, imports: &[JavaImport], uses: &[JavaUse], fields: &[JavaField]) -> Result<(), String> {
        todo!()
    }
}

impl JavaFnSignature {
    fn resolve(&mut self, imports: &[JavaImport], uses: &[JavaUse]) -> Result<(), String> {
        let mut failures = Vec::new();
        if let Err(e) = self.returns.resolve(imports, uses) {
            failures.push(e);
        }
        for param in self.params.iter_mut() {
            if let Err(e) = param.typ.resolve(imports, uses) {
                failures.push(e);
            }
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(failures.join("; "))
        }
    }
}

impl JavaType {
    fn resolve(&mut self, imports: &[JavaImport], uses: &[JavaUse]) -> Result<(), String> {
        match self {
            Self::Array(a) => {
                return a.resolve(imports, uses)
            }
            Self::Object(o) => {
                o.resolve(imports, uses)?;
                if o.to_string() == "java.lang.String" {
                    *self = Self::String;
                }
            }
            _ => {}
        }

        Ok(())
    }
}

impl DottedName {
    fn resolve(&mut self, imports: &[JavaImport], uses: &[JavaUse]) -> Result<(), String> {
        if self.is_dotted() {
            Ok(())
        } else {
            let name = self.to_string();
            for imp in imports {
                if imp.item.name() == &name {
                    *self = imp.item.with_name_token(self.0.pop().unwrap());
                    return Ok(())
                }
            }
            for us in uses {
                if us.java_path.name() == &name {
                    *self = us.java_path.with_name_token(self.0.pop().unwrap());
                    return Ok(())
                }
            }
            Err(format!("Could not resolve class path for '{name}'"))
        }
    }
}

fn resolve_field_signature(obj: &Ident, field: &Ident, signature: &mut Option<JavaSignatureTag>, fields: &[JavaField]) -> Result<(), String> {
    match signature {
        Some(_) => Ok(()),
        None => {
            for f in fields {
                if &f.name == field {
                    let s = f.typ.java_signature();
                    *signature = Some(JavaSignatureTag(parse_quote!(#s)));
                    return Ok(())
                }
            }
            Err(format!("Could not resolve type signature for `{}.{}`", obj, field))
        }
    }
}

fn resolve_static_field_signature(class: &DottedName, field: &Ident, signature: &mut Option<JavaSignatureTag>, fields: &[JavaField]) -> Result<(), String> {
    match signature {
        Some(_) => Ok(()),
        None => {
            for f in fields {
                if &f.name == field {
                    let s = f.typ.java_signature();
                    *signature = Some(JavaSignatureTag(parse_quote!(#s)));
                    return Ok(())
                }
            }
            Err(format!("could not resolve type signature for `{}::{}`", class, field))
        }
    }
}

fn resolve_array_signature(array: &Ident, signature: &mut Option<JavaSignatureTag>) -> Result<(), String> {
    match signature {
        Some(_) => Ok(()),
        None => Err(format!("signature tag is needed for array indexing: `{}[...]`", array)),
    }
}

fn resolve_method_signature(obj: &Ident, method: &Ident, signature: &mut Option<JavaSignatureTag>) -> Result<(), String> {
    match signature {
        Some(_) => Ok(()),
        None => Err(format!("signature tag is needed for method call: `{}.{}(...)`", obj, method)),
    }
}

fn resolve_static_function_signature(class: &DottedName, func: &Ident, signature: &mut Option<JavaSignatureTag>) -> Result<(), String> {
    match signature {
        Some(_) => Ok(()),
        None => Err(format!("signature tag is needed for function call: `{}::{}(...)`", class, func)),
    }
}

impl JavaType {
    fn java_signature(&self) -> Cow<'static, str> {
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
            JavaType::Object(o) => return Cow::Owned(format!("L{o};")),
            JavaType::Array(a) => return Cow::Owned(format!("[{}", a.java_signature())),
        })
    }
}


