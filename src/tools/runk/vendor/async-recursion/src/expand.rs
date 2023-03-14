use crate::parse::{AsyncItem, RecursionArgs};
use proc_macro2::{Span, TokenStream};
use quote::{quote, ToTokens};
use syn::punctuated::Punctuated;
use syn::{parse_quote, Block, FnArg, Lifetime, ReturnType, Signature, Type, WhereClause};

impl ToTokens for AsyncItem {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.0.to_tokens(tokens)
    }
}

pub fn expand(item: &mut AsyncItem, args: &RecursionArgs) {
    transform_sig(&mut item.0.sig, &args);
    transform_block(&mut item.0.block);
}

fn transform_block(block: &mut Block) {
    let brace = block.brace_token;
    *block = parse_quote!({
        Box::pin(async move #block)
    });
    block.brace_token = brace;
}

enum ArgLifetime {
    New(Lifetime),
    Existing(Lifetime),
}

impl ArgLifetime {
    pub fn lifetime(self) -> Lifetime {
        match self {
            ArgLifetime::New(lt) => lt,
            ArgLifetime::Existing(lt) => lt,
        }
    }
}

// Input:
//     async fn f<S, T>(x : S, y : &T) -> Ret;
//
// Output:
//     fn f<S, T>(x : S, y : &T) -> Pin<Box<dyn Future<Output = Ret> + Send>
fn transform_sig(sig: &mut Signature, args: &RecursionArgs) {
    // Determine the original return type
    let ret = match &sig.output {
        ReturnType::Default => quote!(()),
        ReturnType::Type(_, ret) => quote!(#ret),
    };

    // Remove the asyncness of this function
    sig.asyncness = None;

    // Find all reference arguments
    let mut ref_arguments = Vec::new();
    let mut self_lifetime = None;

    for arg in &mut sig.inputs {
        if let FnArg::Typed(pt) = arg {
            if let Type::Reference(tr) = pt.ty.as_mut() {
                ref_arguments.push(tr);
            }
        } else if let FnArg::Receiver(recv) = arg {
            if let Some((_, slt)) = &mut recv.reference {
                self_lifetime = Some(slt);
            }
        }
    }

    let mut counter = 0;
    let mut lifetimes = Vec::new();

    if !ref_arguments.is_empty() {
        for ra in ref_arguments.iter_mut() {
            // If this reference arg doesn't have a lifetime, give it an explicit one
            if ra.lifetime.is_none() {
                let lt = Lifetime::new(&format!("'life{}", counter), Span::call_site());

                lifetimes.push(ArgLifetime::New(parse_quote!(#lt)));

                ra.lifetime = Some(lt);
                counter += 1;
            } else {
                let lt = ra.lifetime.as_ref().cloned().unwrap();

                // Check that this lifetime isn't already in our vector
                let ident_matches = |x: &ArgLifetime| {
                    if let ArgLifetime::Existing(elt) = x {
                        elt.ident == lt.ident
                    } else {
                        false
                    }
                };

                if !lifetimes.iter().any(ident_matches) {
                    lifetimes.push(ArgLifetime::Existing(
                        ra.lifetime.as_ref().cloned().unwrap(),
                    ));
                }
            }
        }
    }

    // Does this expansion require `async_recursion to be added to the output
    let mut requires_lifetime = false;
    let mut where_clause_lifetimes = vec![];
    let mut where_clause_generics = vec![];

    // 'async_recursion lifetime
    let asr: Lifetime = parse_quote!('async_recursion);

    // Add an S : 'async_recursion bound to any generic parameter
    for param in sig.generics.type_params() {
        let ident = param.ident.clone();
        where_clause_generics.push(ident);

        requires_lifetime = true;
    }

    // Add an 'a : 'async_recursion bound to any lifetimes 'a appearing in the function
    if !lifetimes.is_empty() {
        for alt in lifetimes {
            if let ArgLifetime::New(lt) = &alt {
                // If this is a new argument,
                sig.generics.params.push(parse_quote!(#lt));
            }

            // Add a bound to the where clause
            let lt = alt.lifetime();
            where_clause_lifetimes.push(lt);
        }

        requires_lifetime = true;
    }

    // If our function accepts &self, then we modify this to the explicit lifetime &'life_self,
    // and add the bound &'life_self : 'async_recursion
    if let Some(slt) = self_lifetime {
        let lt = {
            if let Some(lt) = slt.as_mut() {
                lt.clone()
            } else {
                // We use `life_self here to avoid any collisions with `life0, `life1 from above
                let lt: Lifetime = parse_quote!('life_self);
                sig.generics.params.push(parse_quote!(#lt));

                // add lt to the lifetime of self
                *slt = Some(lt.clone());

                lt
            }
        };

        where_clause_lifetimes.push(lt);
        requires_lifetime = true;
    }

    let box_lifetime: TokenStream = if requires_lifetime {
        // Add 'async_recursion to our generic parameters
        sig.generics.params.push(parse_quote!('async_recursion));

        quote!(+ #asr)
    } else {
        quote!()
    };

    let send_bound: TokenStream = if args.send_bound {
        quote!(+ ::core::marker::Send)
    } else {
        quote!()
    };

    let where_clause = sig
        .generics
        .where_clause
        .get_or_insert_with(|| WhereClause {
            where_token: Default::default(),
            predicates: Punctuated::new(),
        });

    // Add our S : 'async_recursion bounds
    for generic_ident in where_clause_generics {
        where_clause
            .predicates
            .push(parse_quote!(#generic_ident : #asr));
    }

    // Add our 'a : 'async_recursion bounds
    for lifetime in where_clause_lifetimes {
        where_clause.predicates.push(parse_quote!(#lifetime : #asr));
    }

    // Modify the return type
    sig.output = parse_quote! {
        -> ::core::pin::Pin<Box<
            dyn ::core::future::Future<Output = #ret> #box_lifetime #send_bound >>
    };
}
