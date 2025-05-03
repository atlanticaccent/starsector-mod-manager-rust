use proc_macro::TokenStream;

mod apply_fork;
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
pub fn OptionSpec(attr: TokenStream, item: TokenStream) -> TokenStream {
  invert::Invert(attr, item)
}

#[proc_macro_attribute]
pub fn apply_fork(args: TokenStream, input: TokenStream) -> TokenStream {
  apply_fork::apply(args, input)
}
