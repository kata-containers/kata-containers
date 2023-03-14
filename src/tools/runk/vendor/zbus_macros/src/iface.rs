use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::collections::BTreeMap;
use syn::{
    self, parse_quote, punctuated::Punctuated, AngleBracketedGenericArguments, AttributeArgs,
    Error, FnArg, ImplItem, ItemImpl, Lit::Str, Meta, Meta::NameValue, MetaList, MetaNameValue,
    NestedMeta, PatType, PathArguments, ReturnType, Signature, Token, Type, TypePath,
};

use crate::utils::*;

#[derive(Debug)]
struct Property<'a> {
    read: bool,
    write: bool,
    ty: Option<&'a Type>,
    doc_comments: TokenStream,
}

impl<'a> Property<'a> {
    fn new() -> Self {
        Self {
            read: false,
            write: false,
            ty: None,
            doc_comments: quote!(),
        }
    }
}

pub fn expand(args: AttributeArgs, mut input: ItemImpl) -> syn::Result<TokenStream> {
    let zbus = zbus_path();

    let self_ty = &input.self_ty;
    let mut properties = BTreeMap::new();
    let mut set_dispatch = quote!();
    let mut set_mut_dispatch = quote!();
    let mut get_dispatch = quote!();
    let mut get_all = quote!();
    let mut call_dispatch = quote!();
    let mut call_mut_dispatch = quote!();
    let mut introspect = quote!();
    let mut generated_signals = quote!();

    // the impl Type
    let ty = match input.self_ty.as_ref() {
        Type::Path(p) => {
            &p.path
                .segments
                .last()
                .ok_or_else(|| Error::new_spanned(p, "Unsupported 'impl' type"))?
                .ident
        }
        _ => return Err(Error::new_spanned(&input.self_ty, "Invalid type")),
    };

    let mut iface_name = None;
    for arg in args {
        match arg {
            NestedMeta::Meta(NameValue(nv)) => {
                if nv.path.is_ident("interface") || nv.path.is_ident("name") {
                    if let Str(lit) = nv.lit {
                        iface_name = Some(lit.value());
                    } else {
                        return Err(Error::new_spanned(&nv.lit, "Invalid interface argument"));
                    }
                } else {
                    return Err(Error::new_spanned(&nv.path, "Unsupported argument"));
                }
            }
            _ => return Err(Error::new_spanned(&arg, "Unknown attribute")),
        }
    }
    let iface_name = iface_name.unwrap_or(format!("org.freedesktop.{}", ty));

    for method in &mut input.items {
        let mut method = match method {
            ImplItem::Method(m) => m,
            _ => continue,
        };

        let is_async = method.sig.asyncness.is_some();

        let Signature {
            ident,
            inputs,
            output,
            ..
        } = &mut method.sig;

        let attrs = parse_item_attributes(&method.attrs, "dbus_interface")?;
        method
            .attrs
            .retain(|attr| !attr.path.is_ident("dbus_interface"));
        let docs = get_doc_attrs(&method.attrs)
            .iter()
            .filter_map(|attr| {
                if let Ok(NameValue(MetaNameValue { lit: Str(s), .. })) = attr.parse_meta() {
                    Some(s.value())
                } else {
                    // non #[doc = "..."] attributes are not our concern
                    // we leave them for rustc to handle
                    None
                }
            })
            .collect();

        let doc_comments = to_xml_docs(docs);
        let is_property = attrs.iter().any(|x| x.is_property());
        let is_signal = attrs.iter().any(|x| x.is_signal());
        let out_args = attrs.iter().find(|x| x.is_out_args()).map(|x| match x {
            ItemAttribute::OutArgs(a) => a,
            _ => unreachable!(),
        });
        assert!(!is_property || !is_signal);

        let has_inputs = inputs.len() > 1;

        let is_mut = if let FnArg::Receiver(r) = inputs
            .first()
            .ok_or_else(|| Error::new_spanned(&ident, "not &self method"))?
        {
            r.mutability.is_some()
        } else if is_signal {
            false
        } else {
            return Err(Error::new_spanned(&method, "missing receiver"));
        };
        if is_signal && !is_async {
            return Err(Error::new_spanned(&method, "signals must be async"));
        }
        let method_await = if is_async {
            quote! { .await }
        } else {
            quote! {}
        };

        let mut typed_inputs = inputs
            .iter()
            .filter_map(|i| {
                if let FnArg::Typed(t) = i {
                    Some(t)
                } else {
                    None
                }
            })
            .cloned()
            .collect::<Vec<_>>();
        let signal_context_arg = if is_signal {
            if typed_inputs.is_empty() {
                return Err(Error::new_spanned(
                    &inputs,
                    "Expected a `&zbus::SignalContext<'_> argument",
                ));
            }
            Some(typed_inputs.remove(0))
        } else {
            None
        };

        let mut intro_args = quote!();
        intro_args.extend(introspect_input_args(&typed_inputs, is_signal));
        let is_result_output = introspect_add_output_args(&mut intro_args, output, &out_args)?;

        let (args_from_msg, args) = get_args_from_inputs(&typed_inputs, &zbus)?;

        clean_input_args(inputs);

        let reply = if is_result_output {
            let ret = quote!(r);

            quote!(match reply {
                ::std::result::Result::Ok(r) => c.reply(m, &#ret).await,
                ::std::result::Result::Err(e) => {
                    let hdr = m.header()?;
                    c.reply_dbus_error(&hdr, e).await
                }
            })
        } else {
            quote!(c.reply(m, &reply).await)
        };

        let member_name = attrs
            .iter()
            .find_map(|x| match x {
                ItemAttribute::Name(n) => Some(n.to_string()),
                _ => None,
            })
            .unwrap_or_else(|| {
                let mut name = ident.to_string();
                if is_property && has_inputs {
                    assert!(name.starts_with("set_"));
                    name = name[4..].to_string();
                }
                pascal_case(&name)
            });

        if is_signal {
            introspect.extend(doc_comments);
            introspect.extend(introspect_signal(&member_name, &intro_args));
            let signal_context = signal_context_arg.unwrap().pat;

            method.block = parse_quote!({
                #signal_context.connection().emit_signal(
                    ::std::option::Option::None::<()>,
                    #signal_context.path(),
                    <#self_ty as #zbus::Interface>::name(),
                    #member_name,
                    &(#args),
                )
                .await
            });
        } else if is_property {
            let p = properties.entry(member_name.to_string());
            let prop_changed_method_name = format_ident!("{}_changed", snake_case(&member_name));

            let p = p.or_insert_with(Property::new);
            p.doc_comments.extend(doc_comments);
            if has_inputs {
                p.write = true;

                let set_call = if is_result_output {
                    quote!(self.#ident(val)#method_await)
                } else if is_async {
                    quote!(
                            #zbus::export::futures_util::future::FutureExt::map(
                                self.#ident(val),
                                ::std::result::Result::Ok,
                            )
                            .await
                    )
                } else {
                    quote!(::std::result::Result::Ok(self.#ident(val)))
                };
                let do_set = quote!(
                    match ::std::convert::TryInto::try_into(value) {
                        ::std::result::Result::Ok(val) => {
                            match #set_call {
                                ::std::result::Result::Ok(set_result) => {
                                    self
                                        .#prop_changed_method_name(&signal_context)
                                        .await
                                        .map(|_| set_result)
                                        .map_err(Into::into)
                                }
                                e => e,
                            }
                        }
                        ::std::result::Result::Err(e) => {
                            ::std::result::Result::Err(
                                ::std::convert::Into::into(#zbus::Error::Variant(::std::convert::Into::into(e))),
                            )
                        }
                    }
                );

                if is_mut {
                    let q = quote!(
                        #member_name => {
                            ::std::option::Option::Some(#do_set)
                        }
                    );
                    set_mut_dispatch.extend(q);

                    let q = quote!(
                        #member_name => #zbus::DispatchResult::RequiresMut,
                    );
                    set_dispatch.extend(q);
                } else {
                    let q = quote!(
                        #member_name => {
                            #zbus::DispatchResult::Async(::std::boxed::Box::pin(async move {
                                #do_set
                            }))
                        }
                    );
                    set_dispatch.extend(q);
                }
            } else {
                p.ty = Some(get_property_type(output)?);
                p.read = true;

                let q = quote!(
                    #member_name => {
                        ::std::option::Option::Some(::std::result::Result::Ok(
                            ::std::convert::Into::into(
                                <#zbus::zvariant::Value as ::std::convert::From<_>>::from(
                                    self.#ident()#method_await,
                                ),
                            ),
                        ))
                    }
                );
                get_dispatch.extend(q);

                let q = quote!(
                    props.insert(
                        ::std::string::ToString::to_string(#member_name),
                        ::std::convert::Into::into(
                            <#zbus::zvariant::Value as ::std::convert::From<_>>::from(
                                self.#ident()#method_await,
                            ),
                        ),
                    );
                );
                get_all.extend(q);

                let prop_changed_method = quote!(
                    pub async fn #prop_changed_method_name(
                        &self,
                        signal_context: &#zbus::SignalContext<'_>,
                    ) -> #zbus::Result<()> {
                        let mut changed = ::std::collections::HashMap::new();
                        let value = <#zbus::zvariant::Value as ::std::convert::From<_>>::from(self.#ident()#method_await);
                        changed.insert(#member_name, &value);
                        #zbus::fdo::Properties::properties_changed(
                            signal_context,
                            #zbus::names::InterfaceName::from_static_str_unchecked(#iface_name),
                            &changed,
                            &[],
                        ).await
                    }
                );
                generated_signals.extend(prop_changed_method);
            }
        } else {
            introspect.extend(doc_comments);
            introspect.extend(introspect_method(&member_name, &intro_args));

            let m = quote! {
                #member_name => {
                    let future = async move {
                        #args_from_msg
                        let reply = self.#ident(#args)#method_await;
                        #reply
                    };
                    #zbus::DispatchResult::Async(::std::boxed::Box::pin(async move {
                        future.await.map(|_seq: u32| ())
                    }))
                },
            };

            if is_mut {
                call_dispatch.extend(quote! {
                    #member_name => #zbus::DispatchResult::RequiresMut,
                });
                call_mut_dispatch.extend(m);
            } else {
                call_dispatch.extend(m);
            }
        }
    }

    introspect_properties(&mut introspect, properties)?;

    let generics = &input.generics;
    let where_clause = &generics.where_clause;

    Ok(quote! {
        #input

        impl #generics #self_ty
        #where_clause
        {
            #generated_signals
        }

        #[#zbus::export::async_trait::async_trait]
        impl #generics #zbus::Interface for #self_ty
        #where_clause
        {
            fn name() -> #zbus::names::InterfaceName<'static> {
                #zbus::names::InterfaceName::from_static_str_unchecked(#iface_name)
            }

            async fn get(
                &self,
                property_name: &str,
            ) -> ::std::option::Option<#zbus::fdo::Result<#zbus::zvariant::OwnedValue>> {
                match property_name {
                    #get_dispatch
                    _ => ::std::option::Option::None,
                }
            }

            async fn get_all(
                &self,
            ) -> ::std::collections::HashMap<
                ::std::string::String,
                #zbus::zvariant::OwnedValue,
            > {
                let mut props: ::std::collections::HashMap<
                    ::std::string::String,
                    #zbus::zvariant::OwnedValue,
                > = ::std::collections::HashMap::new();
                #get_all
                props
            }

            fn set<'call>(
                &'call self,
                property_name: &'call str,
                value: &'call #zbus::zvariant::Value<'_>,
                signal_context: &'call #zbus::SignalContext<'_>,
            ) -> #zbus::DispatchResult<'call> {
                match property_name {
                    #set_dispatch
                    _ => #zbus::DispatchResult::NotFound,
                }
            }

            async fn set_mut(
                &mut self,
                property_name: &str,
                value: &#zbus::zvariant::Value<'_>,
                signal_context: &#zbus::SignalContext<'_>,
            ) -> ::std::option::Option<#zbus::fdo::Result<()>> {
                match property_name {
                    #set_mut_dispatch
                    _ => ::std::option::Option::None,
                }
            }

            fn call<'call>(
                &'call self,
                s: &'call #zbus::ObjectServer,
                c: &'call #zbus::Connection,
                m: &'call #zbus::Message,
                name: #zbus::names::MemberName<'call>,
            ) -> #zbus::DispatchResult<'call> {
                match name.as_str() {
                    #call_dispatch
                    _ => #zbus::DispatchResult::NotFound,
                }
            }

            fn call_mut<'call>(
                &'call mut self,
                s: &'call #zbus::ObjectServer,
                c: &'call #zbus::Connection,
                m: &'call #zbus::Message,
                name: #zbus::names::MemberName<'call>,
            ) -> #zbus::DispatchResult<'call> {
                match name.as_str() {
                    #call_mut_dispatch
                    _ => #zbus::DispatchResult::NotFound,
                }
            }

            fn introspect_to_writer(&self, writer: &mut dyn ::std::fmt::Write, level: usize) {
                ::std::writeln!(
                    writer,
                    r#"{:indent$}<interface name="{}">"#,
                    "",
                    <Self as #zbus::Interface>::name(),
                    indent = level
                ).unwrap();
                {
                    use #zbus::zvariant::Type;

                    let level = level + 2;
                    #introspect
                }
                ::std::writeln!(writer, r#"{:indent$}</interface>"#, "", indent = level).unwrap();
            }
        }
    })
}

fn get_args_from_inputs(
    inputs: &[PatType],
    zbus: &TokenStream,
) -> syn::Result<(TokenStream, TokenStream)> {
    if inputs.is_empty() {
        Ok((quote!(), quote!()))
    } else {
        let mut server_arg_decl = None;
        let mut conn_arg_decl = None;
        let mut header_arg_decl = None;
        let mut signal_context_arg_decl = None;
        let mut args = Vec::new();
        let mut tys = Vec::new();

        for input in inputs {
            let mut is_server = false;
            let mut is_conn = false;
            let mut is_header = false;
            let mut is_signal_context = false;

            for attr in &input.attrs {
                if !attr.path.is_ident("zbus") {
                    continue;
                }

                let nested = match attr.parse_meta()? {
                    Meta::List(MetaList { nested, .. }) => nested,
                    meta => {
                        return Err(Error::new_spanned(
                            meta,
                            "Unsupported syntax\n
                             Did you mean `#[zbus(...)]`?",
                        ));
                    }
                };

                for item in nested {
                    match &item {
                        NestedMeta::Meta(Meta::Path(p)) => {
                            if p.is_ident("object_server") {
                                is_server = true;
                            } else if p.is_ident("connection") {
                                is_conn = true;
                            } else if p.is_ident("header") {
                                is_header = true;
                            } else if p.is_ident("signal_context") {
                                is_signal_context = true;
                            } else {
                                return Err(Error::new_spanned(
                                    item,
                                    "Unrecognized zbus attribute",
                                ));
                            }
                        }
                        NestedMeta::Meta(_) => {
                            return Err(Error::new_spanned(item, "Unrecognized zbus attribute"));
                        }
                        NestedMeta::Lit(l) => {
                            return Err(Error::new_spanned(l, "Unexpected literal"))
                        }
                    }
                }
            }

            if is_server {
                if server_arg_decl.is_some() {
                    return Err(Error::new_spanned(
                        input,
                        "There can only be one object_server argument",
                    ));
                }

                let server_arg = &input.pat;
                server_arg_decl = Some(quote! { let #server_arg = &s; });
            } else if is_conn {
                if conn_arg_decl.is_some() {
                    return Err(Error::new_spanned(
                        input,
                        "There can only be one connection argument",
                    ));
                }

                let conn_arg = &input.pat;
                conn_arg_decl = Some(quote! { let #conn_arg = &c; });
            } else if is_header {
                if header_arg_decl.is_some() {
                    return Err(Error::new_spanned(
                        input,
                        "There can only be one header argument",
                    ));
                }

                let header_arg = &input.pat;

                header_arg_decl = Some(quote! {
                    let #header_arg = m.header()?;
                });
            } else if is_signal_context {
                if signal_context_arg_decl.is_some() {
                    return Err(Error::new_spanned(
                        input,
                        "There can only be one `signal_context` argument",
                    ));
                }

                let signal_context_arg = &input.pat;

                signal_context_arg_decl = Some(quote! {
                    let #signal_context_arg = match m.path() {
                        ::std::option::Option::Some(p) => {
                            #zbus::SignalContext::new(c, p).expect("Infallible conversion failed")
                        }
                        ::std::option::Option::None => {
                            let hdr = m.header()?;
                            let err = #zbus::fdo::Error::UnknownObject("Path Required".into());
                            return c.reply_dbus_error(&hdr, err).await;
                        }
                    };
                });
            } else {
                args.push(&input.pat);
                tys.push(&input.ty);
            }
        }

        let args_from_msg = quote! {
            #server_arg_decl

            #conn_arg_decl

            #header_arg_decl

            #signal_context_arg_decl

            let (#(#args),*): (#(#tys),*) =
                match m.body() {
                    ::std::result::Result::Ok(r) => r,
                    ::std::result::Result::Err(e) => {
                        let hdr = m.header()?;
                        let err = <#zbus::fdo::Error as ::std::convert::From<_>>::from(e);
                        return c.reply_dbus_error(&hdr, err).await;
                    }
                };
        };

        let all_args = inputs.iter().map(|t| &t.pat);
        let all_args = quote! { #(#all_args,)* };

        Ok((args_from_msg, all_args))
    }
}

fn clean_input_args(inputs: &mut Punctuated<FnArg, Token![,]>) {
    for input in inputs {
        if let FnArg::Typed(t) = input {
            t.attrs.retain(|attr| !attr.path.is_ident("zbus"));
        }
    }
}

fn introspect_signal(name: &str, args: &TokenStream) -> TokenStream {
    quote!(
        ::std::writeln!(writer, "{:indent$}<signal name=\"{}\">", "", #name, indent = level).unwrap();
        {
            let level = level + 2;
            #args
        }
        ::std::writeln!(writer, "{:indent$}</signal>", "", indent = level).unwrap();
    )
}

fn introspect_method(name: &str, args: &TokenStream) -> TokenStream {
    quote!(
        ::std::writeln!(writer, "{:indent$}<method name=\"{}\">", "", #name, indent = level).unwrap();
        {
            let level = level + 2;
            #args
        }
        ::std::writeln!(writer, "{:indent$}</method>", "", indent = level).unwrap();
    )
}

fn introspect_input_args(
    inputs: &[PatType],
    is_signal: bool,
) -> impl Iterator<Item = TokenStream> + '_ {
    inputs
        .iter()
        .filter_map(move |PatType { pat, ty, attrs, .. }| {
            let is_special_arg = attrs.iter().any(|attr| {
                if !attr.path.is_ident("zbus") {
                    return false;
                }

                let meta = match attr.parse_meta() {
                    ::std::result::Result::Ok(meta) => meta,
                    ::std::result::Result::Err(_) => return false,
                };

                let nested = match meta {
                    Meta::List(MetaList { nested, .. }) => nested,
                    _ => return false,
                };

                let res = nested.iter().any(|nested_meta| {
                    matches!(
                        nested_meta,
                        NestedMeta::Meta(Meta::Path(path))
                        if path.is_ident("object_server") || path.is_ident("connection") || path.is_ident("header") || path.is_ident("signal_context")
                    )
                });

                res
            });
            if is_special_arg {
                return None;
            }

            let arg_name = quote!(#pat).to_string();
            let dir = if is_signal { "" } else { " direction=\"in\"" };
            Some(quote!(
                ::std::writeln!(writer, "{:indent$}<arg name=\"{}\" type=\"{}\"{}/>", "",
                         #arg_name, <#ty>::signature(), #dir, indent = level).unwrap();
            ))
        })
}

fn introspect_output_arg(ty: &Type, arg_name: Option<&String>) -> TokenStream {
    let arg_name = match arg_name {
        Some(name) => format!("name=\"{}\" ", name),
        None => String::from(""),
    };

    quote!(
        ::std::writeln!(writer, "{:indent$}<arg {}type=\"{}\" direction=\"out\"/>", "",
                 #arg_name, <#ty>::signature(), indent = level).unwrap();
    )
}

fn get_result_type(p: &TypePath) -> syn::Result<&Type> {
    if let PathArguments::AngleBracketed(AngleBracketedGenericArguments { args, .. }) = &p
        .path
        .segments
        .last()
        .ok_or_else(|| Error::new_spanned(p, "unsupported result type"))?
        .arguments
    {
        if let Some(syn::GenericArgument::Type(ty)) = args.first() {
            return Ok(ty);
        }
    }

    Err(Error::new_spanned(p, "unhandled Result return"))
}

fn introspect_add_output_args(
    args: &mut TokenStream,
    output: &ReturnType,
    arg_names: &Option<&Vec<String>>,
) -> syn::Result<bool> {
    let mut is_result_output = false;

    if let ReturnType::Type(_, ty) = output {
        let mut ty = ty.as_ref();

        if let Type::Path(p) = ty {
            is_result_output = p
                .path
                .segments
                .last()
                .ok_or_else(|| Error::new_spanned(ty, "unsupported output type"))?
                .ident
                == "Result";
            if is_result_output {
                ty = get_result_type(p)?;
            }
        }

        if let Type::Tuple(t) = ty {
            if let Some(arg_names) = arg_names {
                if t.elems.len() != arg_names.len() {
                    // Turn into error
                    panic!("Number of out arg names different from out args specified")
                }
            }
            for i in 0..t.elems.len() {
                let name = arg_names.map(|names| &names[i]);
                args.extend(introspect_output_arg(&t.elems[i], name));
            }
        } else {
            args.extend(introspect_output_arg(ty, None));
        }
    }

    Ok(is_result_output)
}

fn get_property_type(output: &ReturnType) -> syn::Result<&Type> {
    if let ReturnType::Type(_, ty) = output {
        let ty = ty.as_ref();

        if let Type::Path(p) = ty {
            let is_result_output = p
                .path
                .segments
                .last()
                .ok_or_else(|| Error::new_spanned(ty, "unsupported property type"))?
                .ident
                == "Result";
            if is_result_output {
                return get_result_type(p);
            }
        }

        Ok(ty)
    } else {
        Err(Error::new_spanned(output, "Invalid property getter"))
    }
}

fn introspect_properties(
    introspection: &mut TokenStream,
    properties: BTreeMap<String, Property<'_>>,
) -> syn::Result<()> {
    for (name, prop) in properties {
        let access = if prop.read && prop.write {
            "readwrite"
        } else if prop.read {
            "read"
        } else if prop.write {
            "write"
        } else {
            return Err(Error::new_spanned(
                name,
                "property is neither readable nor writable",
            ));
        };
        let ty = prop.ty.ok_or_else(|| {
            Error::new_spanned(&name, "Write-only properties aren't supported yet")
        })?;

        let doc_comments = prop.doc_comments;
        introspection.extend(quote!(
            #doc_comments
            ::std::writeln!(
                writer,
                "{:indent$}<property name=\"{}\" type=\"{}\" access=\"{}\"/>",
                "", #name, <#ty>::signature(), #access, indent = level,
            ).unwrap();
        ));
    }

    Ok(())
}

pub fn to_xml_docs(lines: Vec<String>) -> TokenStream {
    let mut docs = quote!();

    let mut lines: Vec<&str> = lines
        .iter()
        .skip_while(|s| is_blank(s))
        .flat_map(|s| s.split('\n'))
        .collect();

    while let Some(true) = lines.last().map(|s| is_blank(s)) {
        lines.pop();
    }

    if lines.is_empty() {
        return docs;
    }

    docs.extend(quote!(::std::writeln!(writer, "{:indent$}<!--", "", indent = level).unwrap();));
    for line in lines {
        if !line.is_empty() {
            docs.extend(
                quote!(::std::writeln!(writer, "{:indent$}{}", "", #line, indent = level).unwrap();),
            );
        } else {
            docs.extend(quote!(::std::writeln!(writer, "").unwrap();));
        }
    }
    docs.extend(quote!(::std::writeln!(writer, "{:indent$} -->", "", indent = level).unwrap();));

    docs
}
