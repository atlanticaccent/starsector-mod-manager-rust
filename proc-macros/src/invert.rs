use std::collections::VecDeque;

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, Data, DeriveInput};

#[allow(non_snake_case)]
pub fn Invert(item: TokenStream) -> TokenStream {
  use heck::{ToSnekCase, ToUpperCamelCase};

  let derive_input: DeriveInput = parse_macro_input!(item);

  let DeriveInput {
    attrs,
    ident,
    generics,
    data,
    ..
  } = &derive_input;

  let (impl_generics, type_generics, where_clause) = generics.split_for_impl();

  let Data::Struct(strct) = data else {
    panic!("Must be on a struct")
  };

  let (options, rem): (VecDeque<_>, _) = strct
    .fields
    .iter()
    .map(|field| {
      let extracted = extract_type_from_option(&field.ty).cloned();
      (field.clone(), extracted)
    })
    .partition(|(_, extracted)| extracted.is_some());

  let impls: Vec<_> = options
    .clone()
    .into_iter()
    .map(|(option, option_type)| {
      let option_ident = option.ident.unwrap();
      let option_type = option_type.unwrap();
      let option_attrs = option.attrs;

      let field_iter = rem
        .iter()
        .chain(options.iter().filter(|opt| opt.0.ident.as_ref() != Some(&option_ident)))
        .cloned()
        .map(|(field, _)| (field.ident.unwrap(), field.ty, field.attrs));
      let (other_idents, other_types, other_attrs): (Vec<_>, Vec<_>, Vec<_>) =
        itertools::multiunzip(field_iter);

      let option_ident_str = option_ident.to_string();

      let name = format_ident!(
        "{}Inverse{}",
        option_ident_str.to_upper_camel_case(),
        ident
      );

      let lens_mod = format_ident!("{}_lens", option_ident_str.to_snek_case());
      let lens = format_ident!("invert_on_{}", option_ident_str.to_snek_case());

      quote! {
        #(#attrs)*
        pub struct #name #type_generics #where_clause {
          #(#option_attrs)*
          #option_ident: #option_type,
          #(#(#other_attrs)* #other_idents: #other_types),*
        }

        impl #impl_generics From<&#ident> for Option<#name> #type_generics #where_clause {
          fn from(val: &#ident) -> Option<#name> {
            let #ident {
              #option_ident,
              #(#other_idents),*
            } = val.clone();

            #option_ident.map(|inner| {
              #name {
                #option_ident: inner,
                #(#other_idents),*
              }
            })
          }
        }

        impl #impl_generics std::convert::TryFrom<&Option<#name>> for #ident #type_generics #where_clause {
          type Error = &'static str;

          fn try_from(val: &Option<#name>) -> Result<#ident, Self::Error> {
            let #name {
              #option_ident: inner,
              #(#other_idents),*
            } = val.clone().ok_or("Inner was None")?;

            Ok(#ident {
              #option_ident: Some(inner),
              #(#other_idents),*
            })
          }
        }

        #[allow(non_snake_case)]
        pub mod #lens_mod {
          #[allow(non_camel_case_types)]
          #[derive(Debug, Clone, Copy)]
          pub struct #lens();

          impl druid::lens::Lens<super::#ident, Option<super::#name>> for #lens {
            fn with<V, F: FnOnce(&Option<super::#name>) -> V>(&self, data: &super::#ident, f: F) -> V {
              let inner: Option<super::#name> = data.into();
              f(&inner)
            }

            fn with_mut<V, F: FnOnce(&mut Option<super::#name>) -> V>(&self, data: &mut super::#ident, f: F) -> V {
              use std::convert::TryInto;
              let mut inner: Option<super::#name> = (&*data).into();
              let res = f(&mut inner);

              if let Ok(inner) = (&inner).try_into() {
                *data = inner;
              }

              res
            }
          }
        }

        impl #impl_generics #ident #type_generics #where_clause {
          #[allow(non_upper_case_globals)]
          pub const #lens: #lens_mod::#lens = #lens_mod::#lens();
        }
      }
    })
    .collect();

  quote! {
    #derive_input

    #(#impls)*
  }
  .into()
}

fn extract_type_from_option(ty: &syn::Type) -> Option<&syn::Type> {
  use syn::{GenericArgument, Path, PathArguments, PathSegment};

  fn extract_type_path(ty: &syn::Type) -> Option<&Path> {
    match *ty {
      syn::Type::Path(ref typepath) if typepath.qself.is_none() => Some(&typepath.path),
      _ => None,
    }
  }

  // TODO store (with lazy static) the vec of string
  // TODO maybe optimization, reverse the order of segments
  fn extract_option_segment(path: &Path) -> Option<&PathSegment> {
    let idents_of_path: String = path
      .segments
      .iter()
      .into_iter()
      .map(|segment| format!("{}|", segment.ident))
      .collect();

    const PATHS: &[&str; 3] = &["Option|", "std|option|Option|", "core|option|Option|"];

    PATHS
      .iter()
      .find(|s| &idents_of_path == *s)
      .and_then(|_| path.segments.last())
  }

  extract_type_path(ty)
    .and_then(|path| extract_option_segment(path))
    .and_then(|path_seg| {
      let type_params = &path_seg.arguments;
      // It should have only on angle-bracketed param ("<String>"):
      match *type_params {
        PathArguments::AngleBracketed(ref params) => params.args.first(),
        _ => None,
      }
    })
    .and_then(|generic_arg| match *generic_arg {
      GenericArgument::Type(ref ty) => Some(ty),
      _ => None,
    })
}
