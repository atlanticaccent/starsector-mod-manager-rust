use std::collections::VecDeque;

use heck::ToSnekCase;
use proc_macro::TokenStream;
use quote::{format_ident, quote, ToTokens};
use syn::{
  parse_macro_input, parse_quote, punctuated::Punctuated, Data, DeriveInput, Expr, Meta, Path,
  Token, TypeParam,
};

#[proc_macro_derive(Widget, attributes(widget))]
pub fn impl_widget(input: TokenStream) -> TokenStream {
  let DeriveInput {
    ident,
    attrs,
    mut generics,
    ..
  } = parse_macro_input!(input);

  let data_bound: TypeParam = parse_quote!(T: Clone + druid::Data);
  let widget_bound: TypeParam = parse_quote!(W: druid::Widget<T>);
  if let Some(data) = generics
    .type_params_mut()
    .find(|param| param.ident.to_string() == "T")
  {
    data.bounds.extend(data_bound.bounds);
  }
  if let Some(widget) = generics
    .type_params_mut()
    .find(|param| param.ident.to_string() == "W")
  {
    widget.bounds.extend(widget_bound.bounds)
  }
  let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

  let mut event = None;
  let mut lifecycle = None;
  let mut update = None;
  let mut layout = None;
  let mut paint = None;
  let mut widget_pod = None;

  if let Some(attr) = attrs.iter().find(|attr| attr.path().is_ident("widget")) {
    let list = attr.meta.require_list().unwrap();
    let args = list
      .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
      .unwrap();

    for args in args {
      let name_val = args.require_name_value().unwrap();

      let value = match &name_val.value {
        Expr::Path(expr_path) => {
          let ident = expr_path.path.require_ident().unwrap();
          ident.to_token_stream()
        }
        Expr::Lit(lit) => lit.to_token_stream(),
        _ => panic!(
          "Must be literal naming a method on Self {:?}",
          name_val.value
        ),
      };

      match &name_val.path {
        path if path.is_ident("event") => {
          if value.to_string() == "event" {
            panic!("`event` method implementation cannot be named `event`")
          }
          event = Some(quote! {self.#value(ctx, event, data, env)})
        }
        path if path.is_ident("lifecycle") => {
          if value.to_string() == "lifecycle" {
            panic!("`lifecycle` method implementation cannot be named `lifecycle`")
          }
          lifecycle = Some(quote! {self.#value(ctx, event, data, env)})
        }
        path if path.is_ident("update") => {
          if value.to_string() == "update" {
            panic!("`update` method implementation cannot be named `update`")
          }
          update = Some(quote! {self.#value(ctx, old_data, data, env)})
        }
        path if path.is_ident("layout") => {
          if value.to_string() == "layout" {
            panic!("`layout` method implementation cannot be named `layout`")
          }
          layout = Some(quote! {self.#value(ctx, bc, data, env)})
        }
        path if path.is_ident("paint") => {
          if value.to_string() == "paint" {
            panic!("`paint` method implementation cannot be named `paint`")
          }
          paint = Some(quote! {self.#value(ctx, data, env)})
        }
        path if path.is_ident("widget_pod") => widget_pod = Some(quote! {self.#value}),
        _ => panic!("Must be one of `event`, `lifecycle`, `update`, `layout` or `paint`."),
      };
    }
  }

  let widget_pod = widget_pod.unwrap_or_else(|| quote! {self.widget_pod});
  let event = event.unwrap_or_else(|| {
    quote! {
      #widget_pod.event(ctx, event, data, env)
    }
  });
  let lifecycle = lifecycle.unwrap_or_else(|| {
    quote! {
      #widget_pod.lifecycle(ctx, event, data, env)
    }
  });
  let update = update.unwrap_or_else(|| {
    quote! {
      #widget_pod.update(ctx, data, env)
    }
  });
  let layout = layout.unwrap_or_else(|| {
    quote! {
      #widget_pod.layout(ctx, bc, data, env)
    }
  });
  let paint = paint.unwrap_or_else(|| {
    quote! {
      #widget_pod.paint(ctx, data, env)
    }
  });

  quote! {
    impl #impl_generics druid::Widget<T> for #ident #ty_generics #where_clause {
      fn event(&mut self, ctx: &mut druid::EventCtx, event: &druid::Event, data: &mut T, env: &druid::Env) {
        #event
      }

      fn lifecycle(&mut self, ctx: &mut druid::LifeCycleCtx, event: &druid::LifeCycle, data: &T, env: &druid::Env) {
        #lifecycle
      }

      fn update(&mut self, ctx: &mut druid::UpdateCtx, old_data: &T, data: &T, env: &druid::Env) {
        #update
      }

      fn layout(&mut self, ctx: &mut druid::LayoutCtx, bc: &druid::BoxConstraints, data: &T, env: &druid::Env) -> druid::Size {
        #layout
      }

      fn paint(&mut self, ctx: &mut druid::PaintCtx, data: &T, env: &druid::Env) {
        #paint
      }
    }
  }.into()
}

#[proc_macro]
pub fn icon(item: TokenStream) -> TokenStream {
  let path: Path = parse_macro_input!(item);

  let const_name = &path.segments.last().unwrap().ident;
  let id = path.to_token_stream().to_string();

  quote! {
    pub const #const_name: crate::app::util::icons::icon::Icon = crate::app::util::icons::icon::Icon::new(
      #path,
      #id
    );
  }.into()
}

#[allow(non_snake_case)]
#[proc_macro_attribute]
pub fn Invert(_: TokenStream, item: TokenStream) -> TokenStream {
  use heck::ToUpperCamelCase;

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

    const PATHS: &[&str;3] = &["Option|", "std|option|Option|", "core|option|Option|"];

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
