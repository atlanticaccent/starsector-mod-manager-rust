use proc_macro::TokenStream;

mod icon;
mod invert;
mod widget;

#[proc_macro_derive(Widget, attributes(widget))]
pub fn impl_widget(input: TokenStream) -> TokenStream {
  widget::widget(input)
}

#[proc_macro]
pub fn icon(item: TokenStream) -> TokenStream {
  icon::icon(item)
}

#[allow(non_snake_case)]
#[proc_macro_attribute]
pub fn Invert(_: TokenStream, item: TokenStream) -> TokenStream {
  invert::Invert(item)
}
