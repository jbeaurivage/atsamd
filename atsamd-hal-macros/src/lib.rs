use core::panic;
use std::path::{Path, PathBuf};

use proc_macro::TokenStream as TokenStream2;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    Expr, Lit, LitStr, Token,
};

pub(crate) mod chip;
pub(crate) mod settings;

#[derive(Debug)]
struct PeriphVariantSyntax {
    peripheral: LitStr,
    variant: LitStr,
}

impl Parse for PeriphVariantSyntax {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let attrs = Punctuated::<Expr, Token![,]>::parse_terminated(input)?;
        assert!(attrs.len() == 2);

        let Expr::Lit(peripheral) = attrs[0].clone() else {
            panic!("syntax error!");
        };

        let Lit::Str(peripheral) = &peripheral.lit else {
            panic!("syntax error!");
        };

        let Expr::Assign(variant) = attrs[1].clone() else {
            panic!("syntax error!");
        };

        let Expr::Lit(variant) = variant.right.as_ref() else {
            panic!("syntax error!");
        };

        let Lit::Str(variant) = &variant.lit else {
            panic!("syntax error!");
        };

        let args = PeriphVariantSyntax {
            peripheral: peripheral.clone(),
            variant: variant.clone(),
        };

        dbg!(&args);

        Ok(args)
    }
}

#[derive(Debug)]
struct PeriphVariantConfig {
    peripheral: String,
    variant: String,
}

impl From<PeriphVariantSyntax> for PeriphVariantConfig {
    fn from(value: PeriphVariantSyntax) -> Self {
        Self {
            peripheral: value.peripheral.value(),
            variant: value.variant.value(),
        }
    }
}

#[proc_macro_attribute]
pub fn periph_variant(args: TokenStream2, item: TokenStream2) -> TokenStream2 {
    let _hal_config = settings::Config::new().unwrap();
    let chip = chip::Chip::get_config().unwrap();

    let args = parse_macro_input!(args as PeriphVariantSyntax);
    let periph_config: PeriphVariantConfig = args.into();

    dbg!(&periph_config);

    let variant_matches = chip.peripheral_variant_matches(&periph_config);

    emit_code(item, variant_matches)
}

fn emit_code(item: TokenStream2, emit: bool) -> TokenStream2 {
    if emit {
        item
    } else {
        TokenStream2::new()
    }
}

pub(crate) fn config_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("hal")
        .join("config")
}
