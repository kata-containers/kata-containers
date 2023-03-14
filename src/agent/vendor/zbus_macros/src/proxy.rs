use proc_macro2::{Literal, Span, TokenStream};
use quote::{format_ident, quote, quote_spanned, ToTokens};
use regex::Regex;
use std::collections::HashMap;
use syn::{
    self, fold::Fold, parse_quote, spanned::Spanned, AttributeArgs, Error, FnArg, Ident, ItemTrait,
    NestedMeta, ReturnType, TraitItemMethod, Type,
};

use crate::utils::*;

struct AsyncOpts {
    blocking: bool,
    usage: TokenStream,
    wait: TokenStream,
}

impl AsyncOpts {
    fn new(blocking: bool) -> Self {
        let (usage, wait) = if blocking {
            (quote! {}, quote! {})
        } else {
            (quote! { async }, quote! { .await })
        };
        Self {
            blocking,
            usage,
            wait,
        }
    }
}

pub fn expand(args: AttributeArgs, input: ItemTrait) -> Result<TokenStream, Error> {
    let (mut gen_async, mut gen_blocking) = (true, true);
    let (mut async_name, mut blocking_name) = (None, None);
    let mut iface_name = None;
    let mut default_path = None;
    let mut default_service = None;
    for arg in &args {
        match arg {
            NestedMeta::Meta(syn::Meta::NameValue(nv)) => {
                if nv.path.is_ident("interface") || nv.path.is_ident("name") {
                    if let syn::Lit::Str(lit) = &nv.lit {
                        iface_name = Some(lit.value());
                    } else {
                        return Err(Error::new_spanned(&nv.lit, "invalid interface argument"));
                    }
                } else if nv.path.is_ident("default_path") {
                    if let syn::Lit::Str(lit) = &nv.lit {
                        default_path = Some(lit.value());
                    } else {
                        return Err(Error::new_spanned(&nv.lit, "invalid path argument"));
                    }
                } else if nv.path.is_ident("default_service") {
                    if let syn::Lit::Str(lit) = &nv.lit {
                        default_service = Some(lit.value());
                    } else {
                        return Err(Error::new_spanned(&nv.lit, "invalid service argument"));
                    }
                } else if nv.path.is_ident("async_name") {
                    if let syn::Lit::Str(lit) = &nv.lit {
                        async_name = Some(lit.value());
                    } else {
                        return Err(Error::new_spanned(&nv.lit, "invalid async_name argument"));
                    }
                } else if nv.path.is_ident("blocking_name") {
                    if let syn::Lit::Str(lit) = &nv.lit {
                        blocking_name = Some(lit.value());
                    } else {
                        return Err(Error::new_spanned(
                            &nv.lit,
                            "invalid blocking_name argument",
                        ));
                    }
                } else if nv.path.is_ident("gen_async") {
                    if let syn::Lit::Bool(lit) = &nv.lit {
                        gen_async = lit.value();
                    } else {
                        return Err(Error::new_spanned(&nv.lit, "invalid gen_async argument"));
                    }
                } else if nv.path.is_ident("gen_blocking") {
                    if let syn::Lit::Bool(lit) = &nv.lit {
                        gen_blocking = lit.value();
                    } else {
                        return Err(Error::new_spanned(&nv.lit, "invalid gen_blocking argument"));
                    }
                } else {
                    return Err(Error::new_spanned(&nv.lit, "unsupported argument"));
                }
            }
            _ => return Err(Error::new_spanned(&arg, "unknown attribute")),
        }
    }

    // Some sanity checks
    assert!(
        gen_blocking || gen_async,
        "Can't disable both asynchronous and blocking proxy. 😸",
    );
    assert!(
        gen_blocking || blocking_name.is_none(),
        "Can't set blocking proxy's name if you disabled it. 😸",
    );
    assert!(
        gen_async || async_name.is_none(),
        "Can't set asynchronous proxy's name if you disabled it. 😸",
    );

    let blocking_proxy = if gen_blocking {
        let proxy_name = blocking_name.unwrap_or_else(|| {
            if gen_async {
                format!("{}ProxyBlocking", input.ident)
            } else {
                // When only generating blocking proxy, there is no need for a suffix.
                format!("{}Proxy", input.ident)
            }
        });
        create_proxy(
            &input,
            iface_name.as_deref(),
            default_path.as_deref(),
            default_service.as_deref(),
            &proxy_name,
            true,
            // Signal args structs are shared between the two proxies so always generate it for
            // async proxy only unless async proxy generation is disabled.
            !gen_async,
        )?
    } else {
        quote! {}
    };
    let async_proxy = if gen_async {
        let proxy_name = async_name.unwrap_or_else(|| format!("{}Proxy", input.ident));
        create_proxy(
            &input,
            iface_name.as_deref(),
            default_path.as_deref(),
            default_service.as_deref(),
            &proxy_name,
            false,
            true,
        )?
    } else {
        quote! {}
    };

    Ok(quote! {
        #blocking_proxy

        #async_proxy
    })
}

pub fn create_proxy(
    input: &ItemTrait,
    iface_name: Option<&str>,
    default_path: Option<&str>,
    default_service: Option<&str>,
    proxy_name: &str,
    blocking: bool,
    gen_sig_args: bool,
) -> Result<TokenStream, Error> {
    let zbus = zbus_path();

    let doc = get_doc_attrs(&input.attrs);
    let proxy_name = Ident::new(proxy_name, Span::call_site());
    let ident = input.ident.to_string();
    let name = iface_name
        .map(ToString::to_string)
        .unwrap_or(format!("org.freedesktop.{}", ident));
    let default_path = default_path
        .map(ToString::to_string)
        .unwrap_or(format!("/org/freedesktop/{}", ident));
    let default_service = default_service
        .map(ToString::to_string)
        .unwrap_or_else(|| name.clone());
    let mut methods = TokenStream::new();
    let mut stream_types = TokenStream::new();
    let mut has_properties = false;
    let mut uncached_properties: Vec<String> = vec![];

    let async_opts = AsyncOpts::new(blocking);

    for i in input.items.iter() {
        if let syn::TraitItem::Method(m) = i {
            let method_name = m.sig.ident.to_string();
            let attrs = parse_item_attributes(&m.attrs, "dbus_proxy")?;
            let property_attrs = attrs.iter().find_map(|x| match x {
                ItemAttribute::Property(v) => Some(v),
                _ => None,
            });
            let is_property = property_attrs.is_some();
            let is_signal = attrs.iter().any(|x| x.is_signal());
            let has_inputs = m.sig.inputs.len() > 1;
            let name = attrs
                .iter()
                .find_map(|x| match x {
                    ItemAttribute::Name(n) => Some(n.to_string()),
                    _ => None,
                })
                .unwrap_or_else(|| {
                    pascal_case(if is_property && has_inputs {
                        assert!(method_name.starts_with("set_"));
                        &method_name[4..]
                    } else {
                        &method_name
                    })
                });
            let m = if let Some(prop_attrs) = property_attrs {
                assert!(is_property);
                has_properties = true;
                let emits_changed_signal = PropertyEmitsChangedSignal::parse_from_attrs(prop_attrs);
                if let PropertyEmitsChangedSignal::False = emits_changed_signal {
                    uncached_properties.push(name.clone());
                }
                gen_proxy_property(&name, &method_name, m, &async_opts, emits_changed_signal)
            } else if is_signal {
                let (method, types) = gen_proxy_signal(
                    &proxy_name,
                    &name,
                    &method_name,
                    m,
                    &async_opts,
                    gen_sig_args,
                );
                stream_types.extend(types);

                method
            } else {
                gen_proxy_method_call(&name, &method_name, m, &async_opts)
            };
            methods.extend(m);
        }
    }

    let AsyncOpts { usage, wait, .. } = async_opts;
    let (proxy_struct, connection, builder) = if blocking {
        let connection = quote! { #zbus::blocking::Connection };
        let proxy = quote! { #zbus::blocking::Proxy };
        let builder = quote! { #zbus::blocking::ProxyBuilder };

        (proxy, connection, builder)
    } else {
        let connection = quote! { #zbus::Connection };
        let proxy = quote! { #zbus::Proxy };
        let builder = quote! { #zbus::ProxyBuilder };

        (proxy, connection, builder)
    };

    Ok(quote! {
        impl<'a> #zbus::ProxyDefault for #proxy_name<'a> {
            const INTERFACE: &'static str = #name;
            const DESTINATION: &'static str = #default_service;
            const PATH: &'static str = #default_path;
        }

        #(#doc)*
        #[derive(Clone, Debug)]
        pub struct #proxy_name<'c>(#proxy_struct<'c>);

        impl<'c> #proxy_name<'c> {
            /// Creates a new proxy with the default service & path.
            pub #usage fn new(conn: &#connection) -> #zbus::Result<#proxy_name<'c>> {
                Self::builder(conn).build()#wait
            }

            /// Returns a customizable builder for this proxy.
            pub fn builder(conn: &#connection) -> #builder<'c, Self> {
                let mut builder = #builder::new(conn);
                if #has_properties {
                    let uncached = vec![#(#uncached_properties),*];
                    builder.cache_properties(#zbus::CacheProperties::default())
                           .uncached_properties(&uncached)
                } else {
                    builder.cache_properties(#zbus::CacheProperties::No)
                }
            }

            /// Consumes `self`, returning the underlying `zbus::Proxy`.
            pub fn into_inner(self) -> #proxy_struct<'c> {
                self.0
            }

            /// The reference to the underlying `zbus::Proxy`.
            pub fn inner(&self) -> &#proxy_struct<'c> {
                &self.0
            }

            #methods
        }

        impl<'c> ::std::convert::From<#zbus::Proxy<'c>> for #proxy_name<'c> {
            fn from(proxy: #zbus::Proxy<'c>) -> Self {
                #proxy_name(::std::convert::Into::into(proxy))
            }
        }

        impl<'c> ::std::ops::Deref for #proxy_name<'c> {
            type Target = #proxy_struct<'c>;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl<'c> ::std::ops::DerefMut for #proxy_name<'c> {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.0
            }
        }

        impl<'c> ::std::convert::AsRef<#proxy_struct<'c>> for #proxy_name<'c> {
            fn as_ref(&self) -> &#proxy_struct<'c> {
                &*self
            }
        }

        impl<'c> ::std::convert::AsMut<#proxy_struct<'c>> for #proxy_name<'c> {
            fn as_mut(&mut self) -> &mut #proxy_struct<'c> {
                &mut *self
            }
        }

        impl<'c> #zbus::zvariant::Type for #proxy_name<'c> {
            fn signature() -> #zbus::zvariant::Signature<'static> {
                #zbus::zvariant::OwnedObjectPath::signature()
            }
        }

        impl<'c> #zbus::export::serde::ser::Serialize for #proxy_name<'c> {
            fn serialize<S>(&self, serializer: S) -> ::std::result::Result<S::Ok, S::Error>
            where
                S: #zbus::export::serde::ser::Serializer,
            {
                ::std::string::String::serialize(
                    &::std::string::ToString::to_string(self.inner().path()),
                    serializer,
                )
            }
        }

        #stream_types
    })
}

fn gen_proxy_method_call(
    method_name: &str,
    snake_case_name: &str,
    m: &TraitItemMethod,
    async_opts: &AsyncOpts,
) -> TokenStream {
    let AsyncOpts {
        usage,
        wait,
        blocking,
    } = async_opts;
    let zbus = zbus_path();
    let doc = get_doc_attrs(&m.attrs);
    let args: Vec<_> = m.sig.inputs.iter().filter_map(arg_ident).collect();
    let attrs = parse_item_attributes(&m.attrs, "dbus_proxy").unwrap();
    let async_proxy_object = attrs.iter().find_map(|x| match x {
        ItemAttribute::AsyncObject(o) => Some(o.clone()),
        _ => None,
    });
    let blocking_proxy_object = attrs.iter().find_map(|x| match x {
        ItemAttribute::BlockingObject(o) => Some(o.clone()),
        _ => None,
    });
    let proxy_object = attrs.iter().find_map(|x| match x {
        ItemAttribute::Object(o) => {
            if *blocking {
                // FIXME: for some reason Rust doesn't let us move `blocking_proxy_object` so we've to clone.
                blocking_proxy_object
                    .as_ref()
                    .cloned()
                    .or_else(|| Some(format!("{}ProxyBlocking", o)))
            } else {
                async_proxy_object
                    .as_ref()
                    .cloned()
                    .or_else(|| Some(format!("{}Proxy", o)))
            }
        }
        _ => None,
    });
    let no_reply = attrs.iter().any(|x| matches!(x, ItemAttribute::NoReply));

    let method = Ident::new(snake_case_name, Span::call_site());
    let inputs = &m.sig.inputs;
    let mut generics = m.sig.generics.clone();
    let where_clause = generics.where_clause.get_or_insert(parse_quote!(where));
    for param in generics
        .params
        .iter()
        .filter(|a| matches!(a, syn::GenericParam::Type(_)))
    {
        let is_input_type = inputs.iter().any(|arg| {
            // FIXME: We want to only require `Serialize` from input types and `DeserializeOwned`
            // from output types but since we don't have type introspection, we employ this
            // workaround of regex matching on string reprepresention of the the types to figure out
            // which generic types are input types.
            if let FnArg::Typed(pat) = arg {
                let pattern = format!("& *{}", param.to_token_stream());
                let regex = Regex::new(&pattern).unwrap();
                regex.is_match(&pat.ty.to_token_stream().to_string())
            } else {
                false
            }
        });
        let serde_bound: TokenStream = if is_input_type {
            parse_quote!(#zbus::export::serde::ser::Serialize)
        } else {
            parse_quote!(#zbus::export::serde::de::DeserializeOwned)
        };
        where_clause.predicates.push(parse_quote!(
            #param: #serde_bound + #zbus::zvariant::Type
        ));
    }
    let (_, ty_generics, where_clause) = generics.split_for_impl();

    if let Some(proxy_name) = proxy_object {
        let proxy = Ident::new(&proxy_name, Span::call_site());
        let signature = quote! {
            fn #method#ty_generics(#inputs) -> #zbus::Result<#proxy<'c>>
            #where_clause
        };

        quote! {
            #(#doc)*
            pub #usage #signature {
                let object_path: #zbus::zvariant::OwnedObjectPath =
                    self.0.call(
                        #method_name,
                        &(#(#args),*),
                    )
                    #wait?;
                #proxy::builder(&self.0.connection())
                    .path(object_path)?
                    .build()
                    #wait
            }
        }
    } else {
        let body = if args.len() == 1 {
            // Wrap single arg in a tuple so if it's a struct/tuple itself, zbus will only remove
            // the '()' from the signature that we add and not the actual intended ones.
            let arg = &args[0];
            quote! {
                &(#arg,)
            }
        } else {
            quote! {
                &(#(#args),*)
            }
        };

        let output = &m.sig.output;
        let signature = quote! {
            fn #method#ty_generics(#inputs) #output
            #where_clause
        };

        if no_reply {
            return quote! {
                #(#doc)*
                pub #usage #signature {
                    self.0.call_noreply(#method_name, #body)#wait?;
                    ::std::result::Result::Ok(())
                }
            };
        }

        quote! {
            #(#doc)*
            pub #usage #signature {
                let reply = self.0.call(#method_name, #body)#wait?;
                ::std::result::Result::Ok(reply)
            }
        }
    }
}

/// Standard annotation `org.freedesktop.DBus.Property.EmitsChangedSignal`.
///
/// See <https://dbus.freedesktop.org/doc/dbus-specification.html#introspection-format>.
#[derive(Debug)]
enum PropertyEmitsChangedSignal {
    True,
    Invalidates,
    Const,
    False,
}

impl Default for PropertyEmitsChangedSignal {
    fn default() -> Self {
        PropertyEmitsChangedSignal::True
    }
}

impl PropertyEmitsChangedSignal {
    /// Macro property attribute key, like `#[dbus_proxy(property(emits_changed_signal = "..."))]`.
    const ATTRIBUTE_KEY: &'static str = "emits_changed_signal";

    /// Parse the value from macro attributes.
    fn parse_from_attrs(attrs: &HashMap<String, String>) -> Self {
        attrs
            .get(Self::ATTRIBUTE_KEY)
            .map(|val| match val.as_str() {
                "true" => PropertyEmitsChangedSignal::True,
                "invalidates" => PropertyEmitsChangedSignal::Invalidates,
                "const" => PropertyEmitsChangedSignal::Const,
                "false" => PropertyEmitsChangedSignal::False,
                x => panic!("Invalid attribute '{} = {}'", Self::ATTRIBUTE_KEY, x),
            })
            .unwrap_or_default()
    }
}

fn gen_proxy_property(
    property_name: &str,
    method_name: &str,
    m: &TraitItemMethod,
    async_opts: &AsyncOpts,
    emits_changed_signal: PropertyEmitsChangedSignal,
) -> TokenStream {
    let AsyncOpts {
        usage,
        wait,
        blocking,
    } = async_opts;
    let zbus = zbus_path();
    let doc = get_doc_attrs(&m.attrs);
    let signature = &m.sig;
    if signature.inputs.len() > 1 {
        let value = arg_ident(signature.inputs.last().unwrap()).unwrap();
        quote! {
            #(#doc)*
            #[allow(clippy::needless_question_mark)]
            pub #usage #signature {
                ::std::result::Result::Ok(self.0.set_property(#property_name, #value)#wait?)
            }
        }
    } else {
        // This should fail to compile only if the return type is wrong,
        // so use that as the span.
        let body_span = if let ReturnType::Type(_, ty) = &signature.output {
            ty.span()
        } else {
            signature.span()
        };
        let body = quote_spanned! {body_span =>
            ::std::result::Result::Ok(self.0.get_property(#property_name)#wait?)
        };
        let ret_type = if let ReturnType::Type(_, ty) = &signature.output {
            Some(ty)
        } else {
            None
        };

        let (proxy_name, prop_stream) = if *blocking {
            (
                "zbus::blocking::Proxy",
                quote! { #zbus::blocking::PropertyIterator },
            )
        } else {
            ("zbus::Proxy", quote! { #zbus::PropertyStream })
        };

        let receive_method = match emits_changed_signal {
            PropertyEmitsChangedSignal::True | PropertyEmitsChangedSignal::Invalidates => {
                let (_, ty_generics, where_clause) = m.sig.generics.split_for_impl();
                let receive = format_ident!("receive_{}_changed", method_name);
                let gen_doc = format!(
                    "Create a stream for the `{}` property changes. \
                This is a convenient wrapper around [`{}::receive_property_changed`].",
                    property_name, proxy_name
                );
                quote! {
                    #[doc = #gen_doc]
                    pub #usage fn #receive#ty_generics(
                        &self
                    ) -> #prop_stream<'c, <#ret_type as #zbus::ResultAdapter>::Ok>
                    #where_clause
                    {
                        self.0.receive_property_changed(#property_name)#wait
                    }
                }
            }
            PropertyEmitsChangedSignal::False | PropertyEmitsChangedSignal::Const => {
                quote! {}
            }
        };

        let cached_getter_method = match emits_changed_signal {
            PropertyEmitsChangedSignal::True
            | PropertyEmitsChangedSignal::Invalidates
            | PropertyEmitsChangedSignal::Const => {
                let cached_getter = format_ident!("cached_{}", method_name);
                let cached_doc = format!(
                    " Get the cached value of the `{}` property, or `None` if the property is not cached.",
                    property_name,
                );
                quote! {
                    #[doc = #cached_doc]
                    pub fn #cached_getter(&self) -> ::std::result::Result<
                        ::std::option::Option<<#ret_type as #zbus::ResultAdapter>::Ok>,
                        <#ret_type as #zbus::ResultAdapter>::Err>
                    {
                        self.0.cached_property(#property_name).map_err(::std::convert::Into::into)
                    }
                }
            }
            PropertyEmitsChangedSignal::False => quote! {},
        };

        quote! {
            #(#doc)*
            #[allow(clippy::needless_question_mark)]
            pub #usage #signature {
                #body
            }

            #cached_getter_method

            #receive_method
        }
    }
}

struct SetLifetimeS;

impl Fold for SetLifetimeS {
    fn fold_type_reference(&mut self, node: syn::TypeReference) -> syn::TypeReference {
        let mut t = syn::fold::fold_type_reference(self, node);
        t.lifetime = Some(syn::Lifetime::new("'s", Span::call_site()));
        t
    }

    fn fold_lifetime(&mut self, _node: syn::Lifetime) -> syn::Lifetime {
        syn::Lifetime::new("'s", Span::call_site())
    }
}

fn gen_proxy_signal(
    proxy_name: &Ident,
    signal_name: &str,
    snake_case_name: &str,
    m: &TraitItemMethod,
    async_opts: &AsyncOpts,
    gen_sig_args: bool,
) -> (TokenStream, TokenStream) {
    let AsyncOpts {
        usage,
        wait,
        blocking,
    } = async_opts;
    let zbus = zbus_path();
    let doc = get_doc_attrs(&m.attrs);
    let input_types: Vec<Box<Type>> = m
        .sig
        .inputs
        .iter()
        .filter_map(|arg| match arg {
            FnArg::Typed(p) => Some(p.ty.clone()),
            _ => None,
        })
        .collect();
    let input_types_s: Vec<_> = SetLifetimeS
        .fold_signature(m.sig.clone())
        .inputs
        .iter()
        .filter_map(|arg| match arg {
            FnArg::Typed(p) => Some(p.ty.clone()),
            _ => None,
        })
        .collect();
    let args: Vec<Ident> = m
        .sig
        .inputs
        .iter()
        .filter_map(|arg| arg_ident(arg).cloned())
        .collect();
    let args_nth: Vec<Literal> = args
        .iter()
        .enumerate()
        .map(|(i, _)| Literal::usize_unsuffixed(i))
        .collect();

    let mut generics = m.sig.generics.clone();
    let where_clause = generics.where_clause.get_or_insert(parse_quote!(where));
    for param in generics
        .params
        .iter()
        .filter(|a| matches!(a, syn::GenericParam::Type(_)))
    {
        where_clause
                .predicates
                .push(parse_quote!(#param: #zbus::export::serde::de::Deserialize<'s> + #zbus::zvariant::Type + ::std::fmt::Debug));
    }
    generics.params.push(parse_quote!('s));
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let (proxy_path, receive_signal_link, trait_name, trait_link, signal_type) = if *blocking {
        (
            "zbus::blocking::Proxy",
            "https://docs.rs/zbus/latest/zbus/blocking/struct.Proxy.html#method.receive_signal",
            "Iterator",
            "https://doc.rust-lang.org/std/iter/trait.Iterator.html",
            quote! { blocking::SignalIterator },
        )
    } else {
        (
            "zbus::Proxy",
            "https://docs.rs/zbus/latest/zbus/struct.Proxy.html#method.receive_signal",
            "Stream",
            "https://docs.rs/futures/0.3.15/futures/stream/trait.Stream.html",
            quote! { SignalStream },
        )
    };
    let (receiver_name, stream_name, signal_args, signal_name_ident) = (
        format_ident!("receive_{}", snake_case_name),
        format_ident!("{}{}", signal_name, trait_name),
        format_ident!("{}Args", signal_name),
        format_ident!("{}", signal_name),
    );

    let receive_gen_doc = format!(
        "Create a stream that receives `{}` signals.\n\
            \n\
            This a convenient wrapper around [`{}::receive_signal`]({}).",
        signal_name, proxy_path, receive_signal_link,
    );
    let receive_signal = quote! {
        #[doc = #receive_gen_doc]
        #(#doc)*
        pub #usage fn #receiver_name(&self) -> #zbus::Result<#stream_name<'c>>
        {
            self.receive_signal(#signal_name)#wait.map(#stream_name)
        }
    };

    let stream_gen_doc = format!(
        "A [`{}`] implementation that yields [`{}`] signals.\n\
            \n\
            Use [`{}::receive_{}`] to create an instance of this type.\n\
            \n\
            [`{}`]: {}",
        trait_name, signal_name, proxy_name, snake_case_name, trait_name, trait_link,
    );
    let signal_args_gen_doc = format!("`{}` signal arguments.", signal_name);
    let args_struct_gen_doc = format!("A `{}` signal.", signal_name);
    let args_struct_decl = if gen_sig_args {
        quote! {
            #[doc = #args_struct_gen_doc]
            #[derive(Debug, Clone)]
            pub struct #signal_name_ident(::std::sync::Arc<#zbus::Message>);
        }
    } else {
        quote!()
    };
    let args_impl = if args.is_empty() || !gen_sig_args {
        quote!()
    } else {
        let arg_fields_init = if args.len() == 1 {
            quote! { #(#args)*: args }
        } else {
            quote! { #(#args: args.#args_nth),* }
        };

        quote! {
            impl #signal_name_ident {
                /// Retrieve the signal arguments.
                pub fn args#ty_generics(&'s self) -> #zbus::Result<#signal_args #ty_generics>
                #where_clause
                {
                    self.0.body::<(#(#input_types),*)>()
                        .map_err(::std::convert::Into::into)
                        .map(|args| {
                            #signal_args {
                                phantom: ::std::marker::PhantomData,
                                #arg_fields_init
                            }
                        })
               }
            }

            impl ::std::ops::Deref for #signal_name_ident {
                type Target = #zbus::Message;

                fn deref(&self) -> &#zbus::Message {
                    &self.0
                }
            }

            impl ::std::convert::AsRef<::std::sync::Arc<#zbus::Message>> for #signal_name_ident {
                fn as_ref(&self) -> &::std::sync::Arc<#zbus::Message> {
                    &self.0
                }
            }

            impl ::std::convert::AsRef<#zbus::Message> for #signal_name_ident {
                fn as_ref(&self) -> &#zbus::Message {
                    &self.0
                }
            }

            #[doc = #signal_args_gen_doc]
            pub struct #signal_args #ty_generics {
                phantom: std::marker::PhantomData<&'s ()>,
                #(
                    pub #args: #input_types_s
                 ),*
            }

            impl #impl_generics #signal_args #ty_generics
                #where_clause
            {
                #(
                    pub fn #args(&self) -> &#input_types_s {
                        &self.#args
                    }
                 )*
            }

            impl #impl_generics std::fmt::Debug for #signal_args #ty_generics
                #where_clause
            {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    f.debug_struct(#signal_name)
                    #(
                     .field(stringify!(#args), &self.#args)
                    )*
                     .finish()
                }
            }
        }
    };
    let stream_impl = if *blocking {
        quote! {
            impl ::std::iter::Iterator for #stream_name<'_> {
                type Item = #signal_name_ident;

                fn next(&mut self) -> ::std::option::Option<Self::Item> {
                    ::std::iter::Iterator::next(&mut self.0)
                        .map(#signal_name_ident)
                }
            }
        }
    } else {
        quote! {
            impl #zbus::export::futures_core::stream::Stream for #stream_name<'_> {
                type Item = #signal_name_ident;

                fn poll_next(
                    self: ::std::pin::Pin<&mut Self>,
                    cx: &mut ::std::task::Context<'_>,
                    ) -> ::std::task::Poll<::std::option::Option<Self::Item>> {
                    #zbus::export::futures_core::stream::Stream::poll_next(
                        ::std::pin::Pin::new(&mut self.get_mut().0),
                        cx,
                    )
                    .map(|msg| msg.map(#signal_name_ident))
                }
            }

            impl #zbus::export::ordered_stream::OrderedStream for #stream_name<'_> {
                type Data = #signal_name_ident;
                type Ordering = #zbus::MessageSequence;

                fn poll_next_before(
                    self: ::std::pin::Pin<&mut Self>,
                    cx: &mut ::std::task::Context<'_>,
                    before: ::std::option::Option<&Self::Ordering>
                    ) -> ::std::task::Poll<#zbus::export::ordered_stream::PollResult<Self::Ordering, Self::Data>> {
                    #zbus::export::ordered_stream::OrderedStream::poll_next_before(
                        ::std::pin::Pin::new(&mut self.get_mut().0),
                        cx,
                        before,
                    )
                    .map(|msg| msg.map_data(#signal_name_ident))
                }
            }

            impl #zbus::export::futures_core::stream::FusedStream for #stream_name<'_> {
                fn is_terminated(&self) -> bool {
                    self.0.is_terminated()
                }
            }
        }
    };
    let stream_types = quote! {
        #[doc = #stream_gen_doc]
        #[derive(Debug)]
        pub struct #stream_name<'a>(#zbus::#signal_type<'a>);

        #zbus::export::static_assertions::assert_impl_all!(
            #stream_name<'_>: ::std::marker::Send, ::std::marker::Unpin
        );

        impl<'a> #stream_name<'a> {
            /// Consumes `self`, returning the underlying `zbus::#signal_type`.
            pub fn into_inner(self) -> #zbus::#signal_type<'a> {
                self.0
            }

            /// The reference to the underlying `zbus::#signal_type`.
            pub fn inner(&self) -> & #zbus::#signal_type<'a> {
                &self.0
            }
        }

        impl<'a> std::ops::Deref for #stream_name<'a> {
            type Target = #zbus::#signal_type<'a>;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl ::std::ops::DerefMut for #stream_name<'_> {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.0
            }
        }

        #stream_impl

        #args_struct_decl

        #args_impl
    };

    (receive_signal, stream_types)
}
