//! The in memory stored value for a defined name in a resolved Leo program.

use crate::{
    errors::ValueError,
    types::{Circuit, FieldElement, Function, Identifier, Integer, IntegerType, Type},
};

use snarkos_models::{
    curves::{Field, Group, PrimeField},
    gadgets::utilities::{
        boolean::Boolean, uint128::UInt128, uint16::UInt16, uint32::UInt32, uint64::UInt64,
        uint8::UInt8,
    },
};
use std::fmt;

#[derive(Clone, PartialEq, Eq)]
pub struct ConstrainedCircuitMember<F: Field + PrimeField, G: Group>(
    pub Identifier<F, G>,
    pub ConstrainedValue<F, G>,
);

#[derive(Clone, PartialEq, Eq)]
pub enum ConstrainedValue<F: Field + PrimeField, G: Group> {
    Integer(Integer),
    FieldElement(FieldElement<F>),
    GroupElement(G),
    Boolean(Boolean),

    Array(Vec<ConstrainedValue<F, G>>),

    CircuitDefinition(Circuit<F, G>),
    CircuitExpression(Identifier<F, G>, Vec<ConstrainedCircuitMember<F, G>>),

    Function(Option<Identifier<F, G>>, Function<F, G>), // (optional circuit identifier, function definition)
    Return(Vec<ConstrainedValue<F, G>>),

    Mutable(Box<ConstrainedValue<F, G>>),
    Static(Box<ConstrainedValue<F, G>>),
    Unresolved(String),
}

impl<F: Field + PrimeField, G: Group> ConstrainedValue<F, G> {
    pub(crate) fn expect_type(&self, _type: &Type<F, G>) -> Result<(), ValueError> {
        match (self, _type) {
            (ConstrainedValue::Integer(ref integer), Type::IntegerType(ref _type)) => {
                integer.expect_type(_type)?;
            }
            (ConstrainedValue::FieldElement(ref _f), Type::FieldElement) => {}
            (ConstrainedValue::GroupElement(ref _g), Type::GroupElement) => {}
            (ConstrainedValue::Boolean(ref _b), Type::Boolean) => {}
            (ConstrainedValue::Array(ref arr), Type::Array(ref _type, ref dimensions)) => {
                // check array lengths are equal
                if arr.len() != dimensions[0] {
                    return Err(ValueError::ArrayLength(format!(
                        "Expected array {:?} to be length {:?}",
                        arr, dimensions[0]
                    )));
                }

                // get next dimension of array if nested
                let next_type = _type.next_dimension(dimensions);

                // check each value in array matches
                for value in arr {
                    value.expect_type(&next_type)?;
                }
            }
            (
                ConstrainedValue::CircuitExpression(ref actual_name, ref _members),
                Type::Circuit(ref expected_name),
            ) => {
                if expected_name != actual_name {
                    return Err(ValueError::CircuitName(
                        expected_name.to_string(),
                        actual_name.to_string(),
                    ));
                }
            }
            (
                ConstrainedValue::CircuitExpression(ref actual_name, ref _members),
                Type::SelfType,
            ) => {
                if Identifier::new("Self".into()) == *actual_name {
                    return Err(ValueError::CircuitName(
                        "Self".into(),
                        actual_name.to_string(),
                    ));
                }
            }
            (ConstrainedValue::Return(ref values), _type) => {
                for value in values {
                    value.expect_type(_type)?;
                }
            }
            (ConstrainedValue::Mutable(ref value), _type) => {
                value.expect_type(&_type)?;
            }
            (ConstrainedValue::Static(ref value), _type) => {
                value.expect_type(&_type)?;
            }
            (value, _type) => {
                return Err(ValueError::TypeError(format!(
                    "expected type {}, got {}",
                    _type, value
                )))
            }
        }

        Ok(())
    }

    pub(crate) fn from_other(
        value: String,
        other: &ConstrainedValue<F, G>,
    ) -> Result<Self, ValueError> {
        let other_type = other.to_type();

        ConstrainedValue::from_type(value, &other_type)
    }

    pub(crate) fn from_type(value: String, _type: &Type<F, G>) -> Result<Self, ValueError> {
        Ok(match _type {
            Type::IntegerType(integer_type) => ConstrainedValue::Integer(match integer_type {
                IntegerType::U8 => Integer::U8(UInt8::constant(value.parse::<u8>()?)),
                IntegerType::U16 => Integer::U16(UInt16::constant(value.parse::<u16>()?)),
                IntegerType::U32 => Integer::U32(UInt32::constant(value.parse::<u32>()?)),
                IntegerType::U64 => Integer::U64(UInt64::constant(value.parse::<u64>()?)),
                IntegerType::U128 => Integer::U128(UInt128::constant(value.parse::<u128>()?)),
            }),
            Type::FieldElement => ConstrainedValue::FieldElement(FieldElement::Constant(
                F::from_str(&value).unwrap_or_default(),
            )),
            Type::GroupElement => ConstrainedValue::GroupElement({
                use std::str::FromStr;

                let scalar = G::ScalarField::from_str(&value).unwrap_or_default();
                let point = G::default().mul(&scalar);
                point
            }),
            Type::Boolean => ConstrainedValue::Boolean(Boolean::Constant(value.parse::<bool>()?)),
            _ => ConstrainedValue::Unresolved(value),
        })
    }

    pub(crate) fn to_type(&self) -> Type<F, G> {
        match self {
            ConstrainedValue::Integer(integer) => Type::IntegerType(integer.get_type()),
            ConstrainedValue::FieldElement(_field) => Type::FieldElement,
            ConstrainedValue::GroupElement(_group) => Type::GroupElement,
            ConstrainedValue::Boolean(_bool) => Type::Boolean,
            _ => unimplemented!("to type only implemented for primitives"),
        }
    }
}

impl<F: Field + PrimeField, G: Group> fmt::Display for ConstrainedValue<F, G> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ConstrainedValue::Integer(ref value) => write!(f, "{}", value),
            ConstrainedValue::FieldElement(ref value) => write!(f, "{}", value),
            ConstrainedValue::GroupElement(ref value) => write!(f, "{}", value),
            ConstrainedValue::Boolean(ref value) => write!(f, "{}", value.get_value().unwrap()),
            ConstrainedValue::Array(ref array) => {
                write!(f, "[")?;
                for (i, e) in array.iter().enumerate() {
                    write!(f, "{}", e)?;
                    if i < array.len() - 1 {
                        write!(f, ", ")?;
                    }
                }
                write!(f, "]")
            }
            ConstrainedValue::CircuitExpression(ref identifier, ref members) => {
                write!(f, "{} {{", identifier)?;
                for (i, member) in members.iter().enumerate() {
                    write!(f, "{}: {}", member.0, member.1)?;
                    if i < members.len() - 1 {
                        write!(f, ", ")?;
                    }
                }
                write!(f, "}}")
            }
            ConstrainedValue::Return(ref values) => {
                write!(f, "Program output: [")?;
                for (i, value) in values.iter().enumerate() {
                    write!(f, "{}", value)?;
                    if i < values.len() - 1 {
                        write!(f, ", ")?;
                    }
                }
                write!(f, "]")
            }
            ConstrainedValue::CircuitDefinition(ref _definition) => {
                unimplemented!("cannot return circuit definition in program")
            }
            ConstrainedValue::Function(ref _circuit_option, ref function) => {
                write!(f, "{}();", function.function_name)
            }
            ConstrainedValue::Mutable(ref value) => write!(f, "mut {}", value),
            ConstrainedValue::Static(ref value) => write!(f, "static {}", value),
            ConstrainedValue::Unresolved(ref value) => write!(f, "unresolved {}", value),
        }
    }
}

impl<F: Field + PrimeField, G: Group> fmt::Debug for ConstrainedValue<F, G> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self)
    }
}
