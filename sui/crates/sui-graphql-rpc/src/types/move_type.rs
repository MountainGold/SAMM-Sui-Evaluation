// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::str::FromStr;

use crate::context_data::package_cache::PackageCache;
use async_graphql::*;
use move_core_types::{language_storage::TypeTag, value};
use serde::{Deserialize, Serialize};
use sui_package_resolver::Resolver;

use crate::error::{code, graphql_error};

/// Represents concrete types (no type parameters, no references)
#[derive(SimpleObject, Clone, Debug, PartialEq, Eq)]
#[graphql(complex)]
pub(crate) struct MoveType {
    /// Flat representation of the type signature, as a displayable string.
    repr: String,
}

scalar!(
    MoveTypeSignature,
    "MoveTypeSignature",
    "The signature of a concrete Move Type (a type with all its type parameters instantiated with \
     concrete types, that contains no references), corresponding to the following recursive type:

type MoveTypeSignature =
    \"address\"
  | \"bool\"
  | \"u8\" | \"u16\" | ... | \"u256\"
  | { vector: MoveTypeSignature }
  | {
      struct: {
        package: string,
        module: string,
        type: string,
        typeParameters: [MoveTypeSignature],
      }
    }"
);

scalar!(
    MoveTypeLayout,
    "MoveTypeLayout",
    "The shape of a concrete Move Type (a type with all its type parameters instantiated with \
     concrete types), corresponding to the following recursive type:

type MoveTypeLayout =
    \"address\"
  | \"bool\"
  | \"u8\" | \"u16\" | ... | \"u256\"
  | { vector: MoveTypeLayout }
  | { struct: [{ name: string, layout: MoveTypeLayout }] }"
);

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) enum MoveTypeSignature {
    Address,
    Bool,
    U8,
    U16,
    U32,
    U64,
    U128,
    U256,
    Vector(Box<MoveTypeSignature>),
    Struct {
        package: String,
        module: String,
        #[serde(rename = "type")]
        type_: String,
        type_parameters: Vec<MoveTypeSignature>,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum MoveTypeLayout {
    Address,
    Bool,
    U8,
    U16,
    U32,
    U64,
    U128,
    U256,
    Vector(Box<MoveTypeLayout>),
    Struct(Vec<MoveFieldLayout>),
}

#[derive(Serialize, Deserialize)]
pub(crate) struct MoveFieldLayout {
    name: String,
    layout: MoveTypeLayout,
}

#[ComplexObject]
impl MoveType {
    /// Structured representation of the type signature.
    async fn signature(&self) -> Result<MoveTypeSignature> {
        // Factor out into its own non-GraphQL, non-async function for better testability
        self.signature_impl()
    }

    /// Structured representation of the "shape" of values that match this type.
    async fn layout(&self, ctx: &Context<'_>) -> Result<MoveTypeLayout> {
        let resolver: &Resolver<PackageCache> = ctx.data().map_err(|_| {
            graphql_error(
                code::INTERNAL_SERVER_ERROR,
                "Unable to fetch Package Cache.",
            )
        })?;

        MoveTypeLayout::try_from(self.layout_impl(resolver).await?)
    }
}

impl MoveType {
    pub(crate) fn new(repr: String) -> MoveType {
        Self { repr }
    }

    fn signature_impl(&self) -> Result<MoveTypeSignature> {
        MoveTypeSignature::try_from(self.native_type_tag()?)
    }

    pub(crate) async fn layout_impl(
        &self,
        resolver: &Resolver<PackageCache>,
    ) -> Result<value::MoveTypeLayout> {
        resolver
            .type_layout(self.native_type_tag()?)
            .await
            .map_err(|e| {
                graphql_error(
                    code::INTERNAL_SERVER_ERROR,
                    format!("Error calculating layout for {}: {e}", self.repr),
                )
                .into()
            })
    }

    fn native_type_tag(&self) -> Result<TypeTag> {
        TypeTag::from_str(&self.repr).map_err(|e| {
            graphql_error(
                code::INTERNAL_SERVER_ERROR,
                format!("Error parsing type '{}': {e}", self.repr),
            )
            .into()
        })
    }
}

impl TryFrom<TypeTag> for MoveTypeSignature {
    type Error = async_graphql::Error;

    fn try_from(tag: TypeTag) -> Result<Self> {
        use TypeTag as T;

        Ok(match tag {
            T::Signer => return Err(unexpected_signer_error()),

            T::U8 => Self::U8,
            T::U16 => Self::U16,
            T::U32 => Self::U32,
            T::U64 => Self::U64,
            T::U128 => Self::U128,
            T::U256 => Self::U256,

            T::Bool => Self::Bool,
            T::Address => Self::Address,

            T::Vector(v) => Self::Vector(Box::new(Self::try_from(*v)?)),

            T::Struct(s) => Self::Struct {
                package: s.address.to_canonical_string(/* with_prefix */ true),
                module: s.module.to_string(),
                type_: s.name.to_string(),
                type_parameters: s
                    .type_params
                    .into_iter()
                    .map(Self::try_from)
                    .collect::<Result<Vec<_>>>()?,
            },
        })
    }
}

impl TryFrom<value::MoveTypeLayout> for MoveTypeLayout {
    type Error = async_graphql::Error;

    fn try_from(layout: value::MoveTypeLayout) -> Result<Self> {
        use value::MoveStructLayout as SL;
        use value::MoveTypeLayout as TL;

        Ok(match layout {
            TL::Signer => return Err(unexpected_signer_error()),

            TL::U8 => Self::U8,
            TL::U16 => Self::U16,
            TL::U32 => Self::U32,
            TL::U64 => Self::U64,
            TL::U128 => Self::U128,
            TL::U256 => Self::U256,

            TL::Bool => Self::Bool,
            TL::Address => Self::Address,

            TL::Vector(v) => Self::Vector(Box::new(Self::try_from(*v)?)),

            TL::Struct(SL::Runtime(_)) => {
                return Err(graphql_error(
                    code::INTERNAL_SERVER_ERROR,
                    "Move Struct Layout without field names.",
                )
                .into())
            }

            TL::Struct(SL::WithFields(fields) | SL::WithTypes { fields, .. }) => Self::Struct(
                fields
                    .into_iter()
                    .map(MoveFieldLayout::try_from)
                    .collect::<Result<_>>()?,
            ),
        })
    }
}

impl TryFrom<value::MoveFieldLayout> for MoveFieldLayout {
    type Error = async_graphql::Error;

    fn try_from(layout: value::MoveFieldLayout) -> Result<Self> {
        Ok(Self {
            name: layout.name.to_string(),
            layout: layout.layout.try_into()?,
        })
    }
}

/// Error from seeing a `signer` value or type, which shouldn't be possible in Sui Move.
pub(crate) fn unexpected_signer_error() -> Error {
    graphql_error(
        code::INTERNAL_SERVER_ERROR,
        "Unexpected value of type: signer.",
    )
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    use expect_test::expect;

    fn signature(repr: impl Into<String>) -> Result<MoveTypeSignature> {
        MoveType::new(repr.into()).signature_impl()
    }

    #[test]
    fn complex_type() {
        let sig = signature("vector<0x42::foo::Bar<address, u32, bool, u256>>").unwrap();
        let expect = expect![[r#"
            Vector(
                Struct {
                    package: "0x0000000000000000000000000000000000000000000000000000000000000042",
                    module: "foo",
                    type_: "Bar",
                    type_parameters: [
                        Address,
                        U32,
                        Bool,
                        U256,
                    ],
                },
            )"#]];
        expect.assert_eq(&format!("{sig:#?}"));
    }

    #[test]
    fn tag_parse_error() {
        let err = signature("not_a_type").unwrap_err();
        let expect = expect![[
            r#"Error { message: "Error parsing type 'not_a_type': unexpected token Name(\"not_a_type\"), expected type tag", extensions: None }"#
        ]];
        expect.assert_eq(&format!("{err:?}"));
    }

    #[test]
    fn signer_type() {
        let err = signature("signer").unwrap_err();
        let expect = expect![[
            r#"Error { message: "Unexpected value of type: signer.", extensions: None }"#
        ]];
        expect.assert_eq(&format!("{err:?}"));
    }

    #[test]
    fn nested_signer_type() {
        let err = signature("0x42::baz::Qux<u32, vector<signer>>").unwrap_err();
        let expect = expect![[
            r#"Error { message: "Unexpected value of type: signer.", extensions: None }"#
        ]];
        expect.assert_eq(&format!("{err:?}"));
    }
}
