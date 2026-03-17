use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{Expr, ExprLit, Ident, ItemFn, Lit, LitBool, LitStr, Token};

struct PatchAttr {
    name: LitStr,
    description: Option<LitStr>,
    packages: Vec<PackageEntry>,
    enabled_by_default: Option<bool>,
    depends_on: Vec<LitStr>,
    options: Vec<OptionEntry>,
}

enum PackageEntry {
    Simple(LitStr),
    WithVersions {
        package: LitStr,
        versions: Vec<LitStr>,
    },
}

struct OptionEntry {
    key: LitStr,
    option_type: Ident,
    title: LitStr,
    description: Option<LitStr>,
    default_value: Option<LitStr>,
    valid_values: Vec<LitStr>,
    required: bool,
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

impl Parse for PatchAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut name = None;
        let mut description = None;
        let mut packages = Vec::new();
        let mut enabled_by_default = None;
        let mut depends_on = Vec::new();
        let mut options = Vec::new();

        let entries = Punctuated::<KeyValue, Token![,]>::parse_terminated(input)?;

        for kv in entries {
            match kv.key.to_string().as_str() {
                "name" => name = Some(kv.expect_str()?),
                "description" => description = Some(kv.expect_str()?),
                "packages" => packages = parse_packages(kv.value)?,
                "enabled_by_default" => enabled_by_default = Some(kv.expect_bool()?),
                "depends_on" => depends_on = parse_string_array(kv.value)?,
                "options" => options = parse_options(kv.value)?,
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
            options,
        })
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

/// Parse options array. Each entry is a tuple:
///   ("key", Type, "Title")
///   ("key", Type, "Title", "Description")
///   ("key", Type, "Title", "Description", "default")
///   ("key", Type, "Title", "Description", "default", ["valid1", "valid2"])
///   ("key", Type, "Title", "Description", "default", ["valid1"], required)
///
/// Type is one of: String, Bool, Int, Float, StringList, Path
fn parse_options(expr: Expr) -> syn::Result<Vec<OptionEntry>> {
    let Expr::Array(array) = expr else {
        return Err(syn::Error::new_spanned(expr, "`options` must be an array"));
    };

    let mut entries = Vec::new();
    for elem in array.elems {
        let Expr::Tuple(tuple) = &elem else {
            return Err(syn::Error::new_spanned(
                &elem,
                "each option must be a tuple: (\"key\", Type, \"Title\", ...)",
            ));
        };

        let elems: Vec<_> = tuple.elems.iter().collect();
        if elems.len() < 3 {
            return Err(syn::Error::new_spanned(
                &elem,
                "option tuple must have at least 3 elements: (\"key\", Type, \"Title\")",
            ));
        }

        let key = match &elems[0] {
            Expr::Lit(ExprLit { lit: Lit::Str(s), .. }) => s.clone(),
            _ => return Err(syn::Error::new_spanned(elems[0], "key must be a string")),
        };

        let option_type = match &elems[1] {
            Expr::Path(p) => {
                let ident = p.path.get_ident().ok_or_else(|| {
                    syn::Error::new_spanned(elems[1], "type must be an identifier")
                })?;
                ident.clone()
            }
            _ => return Err(syn::Error::new_spanned(elems[1], "type must be an identifier")),
        };

        let type_str = option_type.to_string();
        match type_str.as_str() {
            "String" | "Bool" | "Int" | "Float" | "StringList" | "Path" => {}
            _ => {
                return Err(syn::Error::new_spanned(
                    &option_type,
                    "type must be one of: String, Bool, Int, Float, StringList, Path",
                ))
            }
        }

        let title = match &elems[2] {
            Expr::Lit(ExprLit { lit: Lit::Str(s), .. }) => s.clone(),
            _ => return Err(syn::Error::new_spanned(elems[2], "title must be a string")),
        };

        let description = if elems.len() > 3 {
            match &elems[3] {
                Expr::Lit(ExprLit { lit: Lit::Str(s), .. }) => Some(s.clone()),
                _ => None,
            }
        } else {
            None
        };

        let default_value = if elems.len() > 4 {
            match &elems[4] {
                Expr::Lit(ExprLit { lit: Lit::Str(s), .. }) => Some(s.clone()),
                _ => None,
            }
        } else {
            None
        };

        let valid_values = if elems.len() > 5 {
            match &elems[5] {
                Expr::Array(arr) => arr
                    .elems
                    .iter()
                    .map(|v| match v {
                        Expr::Lit(ExprLit { lit: Lit::Str(s), .. }) => Ok(s.clone()),
                        _ => Err(syn::Error::new_spanned(v, "valid value must be a string")),
                    })
                    .collect::<syn::Result<Vec<_>>>()?,
                _ => vec![],
            }
        } else {
            vec![]
        };

        let required = if elems.len() > 6 {
            match &elems[6] {
                Expr::Lit(ExprLit { lit: Lit::Bool(b), .. }) => b.value,
                _ => false,
            }
        } else {
            false
        };

        entries.push(OptionEntry {
            key,
            option_type,
            title,
            description,
            default_value,
            valid_values,
            required,
        });
    }
    Ok(entries)
}

fn option_type_tokens(ident: &Ident) -> proc_macro2::TokenStream {
    match ident.to_string().as_str() {
        "String" => quote! { stitch_patch_api::types::OptionType::StringType },
        "Bool" => quote! { stitch_patch_api::types::OptionType::BoolType },
        "Int" => quote! { stitch_patch_api::types::OptionType::IntType },
        "Float" => quote! { stitch_patch_api::types::OptionType::FloatType },
        "StringList" => quote! { stitch_patch_api::types::OptionType::StringListType },
        "Path" => quote! { stitch_patch_api::types::OptionType::PathType },
        _ => quote! { stitch_patch_api::types::OptionType::StringType },
    }
}

fn snake_to_pascal(s: &str) -> String {
    s.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

/// Generates a WASM component patch from an annotated function.
///
/// # Basic usage
/// ```ignore
/// #[stitch_patch(
///     name = "Disable Ads",
///     description = "Hides sponsored ads",
///     packages = ["org.telegram.messenger"],
/// )]
/// fn execute() -> Result<(), String> {
///     // patch logic
///     Ok(())
/// }
/// ```
///
/// # With options
/// ```ignore
/// #[stitch_patch(
///     name = "Custom Theme",
///     description = "Applies custom theme colors",
///     packages = ["org.telegram.messenger"],
///     options = [
///         ("primary_color", String, "Primary Color", "Hex color code", "#FF0000"),
///         ("dark_mode", Bool, "Dark Mode", "Enable dark theme", "true"),
///         ("font_size", Int, "Font Size", "Base font size in sp", "14"),
///         ("icon", Path, "Custom Icon", "Path to launcher icon"),
///         ("style", String, "Style", "Visual style", "modern", ["modern", "classic", "minimal"]),
///     ],
/// )]
/// fn execute() -> Result<(), String> {
///     let color = options::get_string("primary_color").unwrap_or("#FF0000".into());
///     Ok(())
/// }
/// ```
///
/// # With versioned packages and dependencies
/// ```ignore
/// #[stitch_patch(
///     name = "Feature X",
///     description = "Requires base patch",
///     packages = [("org.telegram.messenger", ["11.0.0", "11.1.0"])],
///     depends_on = ["Base Patch"],
///     enabled_by_default = false,
/// )]
/// fn execute() -> Result<(), String> { Ok(()) }
/// ```
///
/// # after_dependents
/// ```ignore
/// #[stitch_patch(name = "Base", description = "Runs after dependents")]
/// fn after_dependents() -> Result<(), String> {
///     // runs after all patches that depend on this one
///     Ok(())
/// }
/// ```
#[proc_macro_attribute]
pub fn stitch_patch(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attrs = syn::parse_macro_input!(attr as PatchAttr);
    let func = syn::parse_macro_input!(item as ItemFn);

    let func_name = &func.sig.ident;
    let struct_name = format_ident!("__{}", snake_to_pascal(&func_name.to_string()));

    let name_str = &attrs.name;
    let desc_str = match &attrs.description {
        Some(d) => quote! { #d.into() },
        None => quote! { String::new() },
    };

    let compat_items: Vec<_> = attrs
        .packages
        .iter()
        .map(|entry| match entry {
            PackageEntry::Simple(pkg) => {
                quote! {
                    stitch_patch_api::Compatibility {
                        package: #pkg.into(),
                        versions: vec![],
                    }
                }
            }
            PackageEntry::WithVersions { package, versions } => {
                quote! {
                    stitch_patch_api::Compatibility {
                        package: #package.into(),
                        versions: vec![#(#versions.into()),*],
                    }
                }
            }
        })
        .collect();

    let enabled = attrs.enabled_by_default.unwrap_or(true);

    let deps: Vec<_> = attrs.depends_on.iter().map(|d| quote! { #d.into() }).collect();

    let is_after = func_name == "after_dependents";
    let (execute_impl, after_impl) = if is_after {
        (
            quote! { fn execute() -> Result<(), String> { Ok(()) } },
            quote! { fn after_dependents() -> Result<(), String> { #func_name() } },
        )
    } else {
        (
            quote! { fn execute() -> Result<(), String> { #func_name() } },
            quote! { fn after_dependents() -> Result<(), String> { Ok(()) } },
        )
    };

    let option_items: Vec<_> = attrs
        .options
        .iter()
        .map(|opt| {
            let key = &opt.key;
            let title = &opt.title;
            let opt_type = option_type_tokens(&opt.option_type);
            let required = opt.required;

            let desc = match &opt.description {
                Some(d) => quote! { #d.into() },
                None => quote! { String::new() },
            };

            let default = match &opt.default_value {
                Some(d) => quote! { Some(#d.into()) },
                None => quote! { None },
            };

            let valid = if opt.valid_values.is_empty() {
                quote! { None }
            } else {
                let vals = &opt.valid_values;
                quote! { Some(vec![#(#vals.into()),*]) }
            };

            quote! {
                stitch_patch_api::OptionDeclaration {
                    key: #key.into(),
                    title: #title.into(),
                    description: #desc,
                    option_type: #opt_type,
                    default_value: #default,
                    valid_values: #valid,
                    required: #required,
                }
            }
        })
        .collect();

    let expanded = quote! {
        #func

        struct #struct_name;

        impl stitch_patch_api::Guest for #struct_name {
            fn metadata() -> stitch_patch_api::PatchMetadata {
                stitch_patch_api::PatchMetadata {
                    name: #name_str.into(),
                    description: #desc_str,
                    compatible_with: vec![#(#compat_items),*],
                    enabled_by_default: #enabled,
                    depends_on: vec![#(#deps),*],
                }
            }

            #execute_impl
            #after_impl

            fn declare_options() -> Vec<stitch_patch_api::OptionDeclaration> {
                vec![#(#option_items),*]
            }
        }

        stitch_patch_api::export!(#struct_name);
    };

    TokenStream::from(expanded)
}
