use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{Expr, ExprLit, Ident, ItemFn, Lit, LitBool, LitStr, Token};

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
        let mut name = None;
        let mut description = None;
        let mut packages = Vec::new();
        let mut enabled_by_default = None;
        let mut depends_on = Vec::new();

        let entries = Punctuated::<KeyValue, Token![,]>::parse_terminated(input)?;

        for kv in entries {
            match kv.key.to_string().as_str() {
                "name" => name = Some(kv.expect_str()?),
                "description" => description = Some(kv.expect_str()?),
                "packages" => packages = parse_packages(kv.value)?,
                "enabled_by_default" => enabled_by_default = Some(kv.expect_bool()?),
                "depends_on" => depends_on = parse_string_array(kv.value)?,
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
                entries.push(PackageEntry::WithVersions { package: pkg, versions });
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

/// Generates a `Patch` trait impl and `stitch_create_patch` FFI export from an annotated function.
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

    let func_name = &func.sig.ident;
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
                static DEPS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
                DEPS.get_or_init(|| vec![#(#deps.to_string()),*])
            }
        }
    };

    let expanded = quote! {
        #func

        struct __StitchPatch {
            #compat_field
        }

        impl Patch for __StitchPatch {
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
        pub extern "C" fn stitch_create_patch() -> *mut Box<dyn Patch> {
            Box::into_raw(Box::new(Box::new(__StitchPatch {
                #compat_init
            }) as Box<dyn Patch>))
        }
    };

    TokenStream::from(expanded)
}
