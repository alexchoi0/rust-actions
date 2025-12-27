use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, ItemFn, FnArg, Type, LitStr};

#[proc_macro_attribute]
pub fn step(attr: TokenStream, item: TokenStream) -> TokenStream {
    let step_name = parse_macro_input!(attr as LitStr);
    let input = parse_macro_input!(item as ItemFn);

    let fn_name = &input.sig.ident;

    let mut params = input.sig.inputs.iter();

    let world_type = match params.next() {
        Some(FnArg::Typed(pat_type)) => {
            extract_world_type(&pat_type.ty)
        }
        _ => {
            return syn::Error::new_spanned(
                &input.sig,
                "Step function must have a world parameter as first argument"
            ).to_compile_error().into();
        }
    };

    let has_args = params.next().is_some();

    let step_call = if has_args {
        quote! {
            let parsed_args = match ::rust_actions::args::FromArgs::from_args(&args) {
                Ok(a) => a,
                Err(e) => return Box::pin(async move { Err(e) }),
            };
            Box::pin(async move {
                let result = #fn_name(world, parsed_args).await?;
                Ok(::rust_actions::outputs::IntoOutputs::into_outputs(result))
            })
        }
    } else {
        quote! {
            Box::pin(async move {
                let result = #fn_name(world).await?;
                Ok(::rust_actions::outputs::IntoOutputs::into_outputs(result))
            })
        }
    };

    let step_name_str = step_name.value();
    let erased_fn_name = syn::Ident::new(
        &format!("__erased_{}", fn_name),
        fn_name.span()
    );

    let expanded = quote! {
        #input

        #[doc(hidden)]
        #[allow(non_upper_case_globals)]
        fn #erased_fn_name<'a>(
            world_any: &'a mut dyn ::std::any::Any,
            args: ::rust_actions::args::RawArgs,
        ) -> ::std::pin::Pin<Box<dyn ::std::future::Future<Output = ::rust_actions::Result<::rust_actions::outputs::StepOutputs>> + Send + 'a>> {
            let world = match world_any.downcast_mut::<#world_type>() {
                Some(w) => w,
                None => {
                    let msg = format!(
                        "World type mismatch: expected {}",
                        ::std::any::type_name::<#world_type>()
                    );
                    return Box::pin(async move {
                        Err(::rust_actions::Error::Custom(msg))
                    });
                }
            };

            #step_call
        }

        ::rust_actions::inventory::submit! {
            ::rust_actions::registry::ErasedStepDef::new(
                #step_name_str,
                {
                    use ::std::any::TypeId;
                    TypeId::of::<#world_type>()
                },
                #erased_fn_name,
            )
        }
    };

    TokenStream::from(expanded)
}

fn extract_world_type(ty: &Type) -> proc_macro2::TokenStream {
    match ty {
        Type::Reference(type_ref) => {
            if let Type::Path(type_path) = &*type_ref.elem {
                let path = &type_path.path;
                quote! { #path }
            } else {
                quote! { compile_error!("Expected a type path for world parameter") }
            }
        }
        _ => {
            quote! { compile_error!("World parameter must be a mutable reference") }
        }
    }
}

#[proc_macro_derive(World, attributes(world))]
pub fn derive_world(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let expanded = quote! {
        impl ::rust_actions::world::World for #name {
            fn new() -> impl ::std::future::Future<Output = ::rust_actions::Result<Self>> + Send {
                Self::setup()
            }
        }
    };

    TokenStream::from(expanded)
}

#[proc_macro_derive(Args, attributes(arg))]
pub fn derive_args(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let expanded = quote! {
        impl ::rust_actions::args::FromArgs for #name {
            fn from_args(args: &::rust_actions::args::RawArgs) -> ::rust_actions::Result<Self> {
                let value = ::rust_actions::serde_json::Value::Object(
                    args.iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect()
                );
                ::rust_actions::serde_json::from_value(value)
                    .map_err(|e| ::rust_actions::Error::Args(e.to_string()))
            }
        }
    };

    TokenStream::from(expanded)
}

#[proc_macro_derive(Outputs)]
pub fn derive_outputs(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let expanded = quote! {
        impl ::rust_actions::outputs::IntoOutputs for #name {
            fn into_outputs(self) -> ::rust_actions::outputs::StepOutputs {
                ::rust_actions::serde_json::to_value(&self)
                    .map(|v| ::rust_actions::outputs::StepOutputs::from_value(v))
                    .unwrap_or_default()
            }
        }
    };

    TokenStream::from(expanded)
}

#[proc_macro_attribute]
pub fn before_all(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    TokenStream::from(quote! { #input })
}

#[proc_macro_attribute]
pub fn after_all(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    TokenStream::from(quote! { #input })
}

#[proc_macro_attribute]
pub fn before_scenario(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    TokenStream::from(quote! { #input })
}

#[proc_macro_attribute]
pub fn after_scenario(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    TokenStream::from(quote! { #input })
}

#[proc_macro_attribute]
pub fn before_step(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    TokenStream::from(quote! { #input })
}

#[proc_macro_attribute]
pub fn after_step(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    TokenStream::from(quote! { #input })
}
