use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{Expr, ExprLit, FnArg, Ident, ItemFn, Lit, LitBool, LitStr, ReturnType, Token, Type};

struct PatchAttr {
    name: LitStr,
    description: Option<LitStr>,
    packages: Vec<PackageEntry>,
    enabled_by_default: Option<bool>,
    depends_on: Vec<LitStr>,
}

enum PackageEntry {
    Simple(LitStr),
    WithVersions {
        package: LitStr,
        versions: Vec<LitStr>,
    },
}

impl Parse for PatchAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut name: Option<LitStr> = None;
        let mut description: Option<LitStr> = None;
        let mut packages = Vec::new();
        let mut enabled_by_default = None;
        let mut depends_on = Vec::new();

        let entries = Punctuated::<KeyValue, Token![,]>::parse_terminated(input)?;

        for kv in entries {
            let key_str = kv.key.to_string();
            match key_str.as_str() {
                "name" => {
                    if name.is_some() {
                        return Err(syn::Error::new(kv.key.span(), "duplicate `name` attribute"));
                    }
                    name = Some(kv.expect_str()?);
                }
                "description" => {
                    if description.is_some() {
                        return Err(syn::Error::new(
                            kv.key.span(),
                            "duplicate `description` attribute",
                        ));
                    }
                    description = Some(kv.expect_str()?);
                }
                "packages" => {
                    if !packages.is_empty() {
                        return Err(syn::Error::new(
                            kv.key.span(),
                            "duplicate `packages` attribute",
                        ));
                    }
                    packages = parse_packages(kv.value)?;
                }
                "enabled_by_default" => {
                    if enabled_by_default.is_some() {
                        return Err(syn::Error::new(
                            kv.key.span(),
                            "duplicate `enabled_by_default` attribute",
                        ));
                    }
                    enabled_by_default = Some(kv.expect_bool()?);
                }
                "depends_on" => {
                    if !depends_on.is_empty() {
                        return Err(syn::Error::new(
                            kv.key.span(),
                            "duplicate `depends_on` attribute",
                        ));
                    }
                    depends_on = parse_string_array(kv.value)?;
                }
                other => {
                    return Err(syn::Error::new(
                        kv.key.span(),
                        format!("unknown attribute: {other}"),
                    ));
                }
            }
        }

        Ok(PatchAttr {
            name: name.ok_or_else(|| input.error("missing `name` attribute"))?,
            description,
            packages,
            enabled_by_default,
            depends_on,
        })
    }
}

struct KeyValue {
    key: Ident,
    value: Expr,
}

impl Parse for KeyValue {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let key: Ident = input.parse()?;
        let _: Token![=] = input.parse()?;
        let value: Expr = input.parse()?;
        Ok(KeyValue { key, value })
    }
}

impl KeyValue {
    fn expect_str(&self) -> syn::Result<LitStr> {
        if let Expr::Lit(ExprLit {
            lit: Lit::Str(s), ..
        }) = &self.value
        {
            Ok(s.clone())
        } else {
            Err(syn::Error::new(
                self.key.span(),
                format!("`{}` must be a string literal", self.key),
            ))
        }
    }

    fn expect_bool(&self) -> syn::Result<bool> {
        if let Expr::Lit(ExprLit {
            lit: Lit::Bool(LitBool { value, .. }),
            ..
        }) = &self.value
        {
            Ok(*value)
        } else {
            Err(syn::Error::new(
                self.key.span(),
                format!("`{}` must be a bool literal", self.key),
            ))
        }
    }
}

fn parse_string_array(expr: Expr) -> syn::Result<Vec<LitStr>> {
    let Expr::Array(array) = expr else {
        return Err(syn::Error::new_spanned(expr, "expected an array of strings"));
    };
    array
        .elems
        .into_iter()
        .map(|elem| match elem {
            Expr::Lit(ExprLit {
                lit: Lit::Str(s), ..
            }) => Ok(s),
            _ => Err(syn::Error::new_spanned(elem, "expected a string literal")),
        })
        .collect()
}

fn parse_packages(expr: Expr) -> syn::Result<Vec<PackageEntry>> {
    let Expr::Array(array) = expr else {
        return Err(syn::Error::new_spanned(
            expr,
            "`packages` must be an array",
        ));
    };

    let mut entries = Vec::new();
    for elem in array.elems {
        match &elem {
            Expr::Lit(ExprLit {
                lit: Lit::Str(s), ..
            }) => {
                entries.push(PackageEntry::Simple(s.clone()));
            }
            Expr::Tuple(tuple) => {
                if tuple.elems.len() != 2 {
                    return Err(syn::Error::new_spanned(
                        &elem,
                        "package tuple must have exactly 2 elements: (package, [versions])",
                    ));
                }
                let mut iter = tuple.elems.iter();
                let pkg = match iter.next() {
                    Some(Expr::Lit(ExprLit {
                        lit: Lit::Str(s), ..
                    })) => s.clone(),
                    _ => {
                        return Err(syn::Error::new_spanned(
                            &elem,
                            "first element must be a package string",
                        ))
                    }
                };
                let versions = match iter.next() {
                    Some(Expr::Array(arr)) => arr
                        .elems
                        .iter()
                        .map(|v| match v {
                            Expr::Lit(ExprLit {
                                lit: Lit::Str(s), ..
                            }) => Ok(s.clone()),
                            _ => Err(syn::Error::new_spanned(v, "version must be a string")),
                        })
                        .collect::<syn::Result<Vec<_>>>()?,
                    _ => {
                        return Err(syn::Error::new_spanned(
                            &elem,
                            "second element must be a version array",
                        ))
                    }
                };
                entries.push(PackageEntry::WithVersions {
                    package: pkg,
                    versions,
                });
            }
            _ => {
                return Err(syn::Error::new_spanned(
                    elem,
                    "entry must be a string or (string, [versions]) tuple",
                ));
            }
        }
    }
    Ok(entries)
}

fn validate_signature(func: &ItemFn) -> syn::Result<()> {
    let params: Vec<_> = func.sig.inputs.iter().collect();
    if params.len() != 1 {
        return Err(syn::Error::new_spanned(
            &func.sig,
            "patch function must take exactly one parameter: &mut PatchContext",
        ));
    }

    if let FnArg::Receiver(_) = &params[0] {
        return Err(syn::Error::new_spanned(
            &params[0],
            "patch function cannot be a method (no `self` parameter)",
        ));
    }

    match &func.sig.output {
        ReturnType::Default => {
            return Err(syn::Error::new_spanned(
                &func.sig,
                "patch function must return Result<()>",
            ));
        }
        ReturnType::Type(_, ty) => {
            if let Type::Path(tp) = ty.as_ref() {
                let last = tp.path.segments.last();
                if !last.is_some_and(|s| s.ident == "Result") {
                    return Err(syn::Error::new_spanned(
                        ty,
                        "patch function must return Result<()>",
                    ));
                }
            }
        }
    }

    Ok(())
}

/// Generates a `Patch` trait impl and FFI export from an annotated function.
///
/// Each `#[stitch_patch]` function compiles into its own shared library (.so/.dylib).
/// The generated FFI symbol is derived from the function name to avoid collisions.
///
/// # Example
/// ```ignore
/// #[stitch_patch(
///     name = "Disable Ads",
///     description = "Hides sponsored ads",
///     packages = ["org.telegram.messenger"]
/// )]
/// fn execute(ctx: &mut PatchContext) -> Result<()> {
///     ctx.method_mut("Lorg/telegram/ui/ChatActivity;", "addSponsoredMessages")?
///         .return_early();
///     Ok(())
/// }
/// ```
#[proc_macro_attribute]
pub fn stitch_patch(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attrs = syn::parse_macro_input!(attr as PatchAttr);
    let func = syn::parse_macro_input!(item as ItemFn);

    if let Err(e) = validate_signature(&func) {
        return e.to_compile_error().into();
    }

    let func_name = &func.sig.ident;
    let struct_name = format_ident!("__StitchPatch_{}", func_name);
    let ffi_name = format_ident!("stitch_create_patch_{}", func_name);
    let name_str = &attrs.name;

    let desc_impl = match &attrs.description {
        Some(d) => quote! { fn description(&self) -> &str { #d } },
        None => quote! {},
    };

    let compat_items: Vec<_> = attrs
        .packages
        .iter()
        .map(|entry| match entry {
            PackageEntry::Simple(pkg) => {
                quote! { Compatibility::package(#pkg) }
            }
            PackageEntry::WithVersions { package, versions } => {
                quote! {
                    Compatibility::with_versions(#package, vec![#(#versions.into()),*])
                }
            }
        })
        .collect();

    let compat_field = if compat_items.is_empty() {
        quote! {}
    } else {
        quote! { compat: Vec<Compatibility>, }
    };

    let compat_init = if compat_items.is_empty() {
        quote! {}
    } else {
        quote! { compat: vec![#(#compat_items),*], }
    };

    let compat_impl = if compat_items.is_empty() {
        quote! {}
    } else {
        quote! {
            fn compatible_with(&self) -> &[Compatibility] {
                &self.compat
            }
        }
    };

    let enabled_impl = match attrs.enabled_by_default {
        Some(v) => quote! { fn enabled_by_default(&self) -> bool { #v } },
        None => quote! {},
    };

    let deps = &attrs.depends_on;
    let deps_impl = if deps.is_empty() {
        quote! {}
    } else {
        quote! {
            fn depends_on(&self) -> &[String] {
                &self.deps
            }
        }
    };

    let deps_field = if deps.is_empty() {
        quote! {}
    } else {
        quote! { deps: Vec<String>, }
    };

    let deps_init = if deps.is_empty() {
        quote! {}
    } else {
        quote! { deps: vec![#(#deps.to_string()),*], }
    };

    let expanded = quote! {
        #func

        struct #struct_name {
            #compat_field
            #deps_field
        }

        impl Patch for #struct_name {
            fn name(&self) -> &str { #name_str }
            #desc_impl
            #compat_impl
            #enabled_impl
            #deps_impl

            fn execute(&self, ctx: &mut PatchContext) -> Result<()> {
                #func_name(ctx)
            }
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn #ffi_name() -> *mut Box<dyn Patch> {
            Box::into_raw(Box::new(Box::new(#struct_name {
                #compat_init
                #deps_init
            }) as Box<dyn Patch>))
        }
    };

    TokenStream::from(expanded)
}
