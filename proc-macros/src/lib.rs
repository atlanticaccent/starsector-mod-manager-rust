use std::collections::VecDeque;

use proc_macro::TokenStream;
use quote::{format_ident, quote, ToTokens};
use syn::{
  parse_macro_input, parse_quote, punctuated::Punctuated, Data, DeriveInput, Expr, Ident, Meta,
  Path, Token, Type, TypeParam,
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

#[proc_macro_derive(Invert)]
pub fn impl_invert(input: TokenStream) -> TokenStream {
  use heck::ToUpperCamelCase;

  let DeriveInput {
    ident,
    generics,
    data,
    ..
  } = parse_macro_input!(input);

  let (_, type_generics, where_clause) = generics.split_for_impl();

  let Data::Struct(strct) = data else {
    panic!("Must be on a struct")
  };

  let (mut options, rem): (VecDeque<_>, _) = strct
    .fields
    .into_iter()
    .map(|field| {
      let extracted = extract_type_from_option(&field.ty).cloned();
      (field, extracted)
    })
    .partition(|(_, extracted)| extracted.is_some());

  // TODO: Either repeat following for all entries in options individually, _or_ only for fields named in attrs

  // assert!(options.len() <= 1, "More than one field wrapped in Option");
  let option = options.pop_front().expect("No fields wrapped in Option");
  let option_ident = option.0.ident.unwrap();
  let option_type = option.1.unwrap();

  let (other_idents, other_types): (Vec<Ident>, Vec<Type>) = rem
    .into_iter()
    .chain(options.into_iter()) // TODO: remove this for future flexible version
    .map(|(field, _)| (field.ident.unwrap(), field.ty))
    .unzip();

  let name = format_ident!(
    "{}Inverse{}",
    option_ident.to_string().to_upper_camel_case(),
    ident
  );

  quote! {
    pub struct #name #type_generics #where_clause {
      #option_ident: #option_type,
      #(#other_idents: #other_types),*
    }
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
    let idents_of_path = path
      .segments
      .iter()
      .into_iter()
      .fold(String::new(), |mut acc, v| {
        acc.push_str(&v.ident.to_string());
        acc.push('|');
        acc
      });
    vec!["Option|", "std|option|Option|", "core|option|Option|"]
      .into_iter()
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
