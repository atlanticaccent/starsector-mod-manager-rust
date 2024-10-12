use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::{
  parse_macro_input, parse_quote, punctuated::Punctuated, DeriveInput, Expr, Meta, Path, Token,
  TypeParam,
};

mod invert;

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
  if let Some(data) = generics.type_params_mut().find(|param| param.ident == "T") {
    data.bounds.extend(data_bound.bounds);
  }
  if let Some(widget) = generics.type_params_mut().find(|param| param.ident == "W") {
    widget.bounds.extend(widget_bound.bounds);
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
        Expr::Lit(literal) => literal.to_token_stream(),
        _ => panic!(
          "Must be literal naming a method on Self {:?}",
          name_val.value
        ),
      };

      match &name_val.path {
        path if path.is_ident("event") => {
          assert_ne!(
            value.to_string(),
            "event",
            "`event` method implementation cannot be named `event`"
          );
          event = Some(quote! {self.#value(ctx, event, data, env)});
        }
        path if path.is_ident("lifecycle") => {
          assert_ne!(
            value.to_string(),
            "lifecycle",
            "`lifecycle` method implementation cannot be named `lifecycle`"
          );
          lifecycle = Some(quote! {self.#value(ctx, event, data, env)});
        }
        path if path.is_ident("update") => {
          assert_ne!(
            value.to_string(),
            "update",
            "`update` method implementation cannot be named `update`"
          );
          update = Some(quote! {self.#value(ctx, old_data, data, env)});
        }
        path if path.is_ident("layout") => {
          assert_ne!(
            value.to_string(),
            "layout",
            "`layout` method implementation cannot be named `layout`"
          );
          layout = Some(quote! {self.#value(ctx, bc, data, env)});
        }
        path if path.is_ident("paint") => {
          assert_ne!(
            value.to_string(),
            "paint",
            "`paint` method implementation cannot be named `paint`"
          );
          paint = Some(quote! {self.#value(ctx, data, env)});
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
  invert::Invert(item)
}
