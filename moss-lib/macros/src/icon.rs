use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::{parse::Parser, punctuated::Punctuated, Path, Token};

pub(crate) fn icon(input: TokenStream) -> TokenStream {
  let parser = Punctuated::<Path, Token![,]>::parse_terminated;
  let components = parser.parse(input).unwrap();

  let [path, icon_type]: [Path; 2] = components
    .into_pairs()
    .map(|pair| pair.into_value())
    .collect::<Vec<_>>()
    .try_into()
    .unwrap();

  let const_name = &path.segments.last().unwrap().ident;
  let id = path.to_token_stream().to_string();

  quote! {
    pub const #const_name: #icon_type = #icon_type::new(
      #path,
      #id
    );
  }
  .into()
}
