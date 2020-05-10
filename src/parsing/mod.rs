
use std::fmt::Formatter;

use nom::Err as NomErr;
use nom::sequence::delimited;
use nom::IResult;
use nom::bytes::complete::take_while1;
use nom::bytes::complete::tag;
use nom::character::complete::alphanumeric0;
use nom::error::VerboseError;
use nom::error::convert_error;
use nom::combinator::recognize;
use nom::character::complete::multispace0;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use nom::branch::alt;
use nom::sequence::preceded;
use nom::sequence::terminated;
use nom::bytes::complete::take_until;
use nom::multi::many0_count;
use nom::combinator::value;
use nom::character::complete::char;
use nom::error::VerboseErrorKind;
use nom::multi::many0;
use nom::multi::many1;
use nom::sequence::tuple;
use nom::combinator::opt;
use nom::character::complete::alphanumeric1;
use nom::bytes::complete::take_while;
use nom::character::is_alphanumeric;
use nom::character::complete::alpha1;

// All tests are kept in their own module.
#[cfg(test)]
mod tests;

pub type ParserResult<'a, O> = IResult<&'a str, O, VerboseError<&'a str>>;

#[derive(PartialOrd, PartialEq, Debug)]
pub enum NLType<'a> {
    None,
    Boolean,
    I8, I16, I32, I64,
    U8, U16, U32, U64,
    F32, F64,
    OwnedString,
    BorrowedString,
    Tuple(Vec<NLType<'a>>),
    OwnedStruct(&'a str),
    ReferencedStruct(&'a str),
    MutableReferencedStruct(&'a str),
    OwnedTrait(&'a str),
    ReferencedTrait(&'a str),
    MutableReferencedTrait(&'a str),
    SelfReference,
    MutableSelfReference,
}

pub struct NLStructVariable<'a> {
    name: &'a str,
    my_type: NLType<'a>,
}

impl<'a> NLStructVariable<'a> {
    pub fn get_name(&self) -> &str { &self.name }
    pub fn get_type(&self) -> &NLType { &self.my_type }
}

pub struct NLArgument<'a> {
    name: &'a str,
    nl_type: NLType<'a>,
}

impl<'a> NLArgument<'a> {
    pub fn get_name(&self) -> &str { &self.name }
    pub fn get_type(&self) -> &NLType { &self.nl_type }
}

#[derive(PartialOrd, PartialEq, Debug)]
pub struct NLBlock<'a> {
    operations: Vec<NLOperation<'a>>,
}

pub struct NLFunction<'a> {
    name: &'a str,
    arguments: Vec<NLArgument<'a>>,
    return_type: NLType<'a>,
    block: Option<NLBlock<'a>>,
}

pub enum NLImplementor<'a> {
    Method(NLFunction<'a>),
    Getter(NLGetter<'a>),
    Setter(NLSetter<'a>),
}

impl<'a> NLFunction<'a> {
    pub fn get_name(&self) -> &str { &self.name }
    pub fn get_arguments(&self) -> &Vec<NLArgument> { &self.arguments }
    pub fn get_return_type(&self) -> &NLType { &self.return_type }
    pub fn get_block(&self) -> &Option<NLBlock> { &self.block }
}

#[derive(PartialOrd, PartialEq, Debug)]
pub enum NLEncapsulationBlock<'a> {
    Some(NLBlock<'a>),
    None,
    Default,
}

pub struct NLGetter<'a> {
    name: String,
    args: Vec<NLArgument<'a>>,
    nl_type: NLType<'a>,
    block: NLEncapsulationBlock<'a>,
}

impl<'a> NLGetter<'a> {
    pub fn get_name(&self) -> &str { &self.name }
    pub fn get_arguments(&self) -> &Vec<NLArgument> { &self.args }
    pub fn get_type(&self) -> &NLType { &self.nl_type }
    pub fn get_block(&self) -> &NLEncapsulationBlock { &self.block }
}

pub struct NLSetter<'a> {
    name: &'a str,
    args: Vec<NLArgument<'a>>,
    block: NLEncapsulationBlock<'a>,
}

impl<'a> NLSetter<'a> {
    pub fn get_name(&self) -> &str { &self.name }
    pub fn get_arguments(&self) -> &Vec<NLArgument> { &self.args }
    pub fn get_block(&self) -> &NLEncapsulationBlock { &self.block }
}

pub struct NLStruct<'a> {
    name: &'a str,
    variables: Vec<NLStructVariable<'a>>,
    implementations: Vec<NLImplementation<'a>>,
}

impl<'a> NLStruct<'a> {
    pub fn get_name(&self) -> &str { &self.name }
    pub fn get_variables(&self) -> &Vec<NLStructVariable> { &self.variables }
    pub fn get_implementations(&self) -> &Vec<NLImplementation> { &self.implementations }
}

pub struct NLTrait<'a> {
    name: &'a str,
    implementors: Vec<NLImplementor<'a>>,
}

impl<'a> NLTrait<'a> {
    pub fn get_name(&self) -> &str { &self.name }
    pub fn get_implementors(&self) -> &Vec<NLImplementor> { &self.implementors }
}

pub struct NLImplementation<'a> {
    name: &'a str,
    implementors: Vec<NLImplementor<'a>>,
}

impl<'a> NLImplementation<'a> {
    pub fn get_name(&self) -> &str { &self.name }
    pub fn get_implementors(&self) -> &Vec<NLImplementor> { &self.implementors }
}

enum RootDeceleration<'a> {
    Struct(NLStruct<'a>),
    Trait(NLTrait<'a>),
    Function(NLFunction<'a>),
}

#[derive(PartialOrd, PartialEq, Debug)]
enum OpConstant<'a> {
    Boolean(bool),
    Integer(i64, NLType<'a>),
    Float(f64, NLType<'a>),
    String(&'a str),
}

#[derive(PartialOrd, PartialEq, Debug)]
struct OpVariable<'a> {
    name: &'a str,
}

#[derive(PartialOrd, PartialEq, Debug)]
struct OpAssignment<'a> {
    is_new: bool,
    to_assign: Vec<OpVariable<'a>>,
    type_assignment: NLType<'a>,
    assignment: Box<NLOperation<'a>>,
}

#[derive(PartialOrd, PartialEq, Debug)]
enum OpOperator<'a> {
    CompareEqual(Box<NLOperation<'a>>, Box<NLOperation<'a>>),
    CompareNotEqual(Box<NLOperation<'a>>, Box<NLOperation<'a>>),
    CompareGreater(Box<NLOperation<'a>>, Box<NLOperation<'a>>),
    CompareLess(Box<NLOperation<'a>>, Box<NLOperation<'a>>),
    CompareGreaterEqual(Box<NLOperation<'a>>, Box<NLOperation<'a>>),
    CompareLessEqual(Box<NLOperation<'a>>, Box<NLOperation<'a>>),

    LogicalNegate(Box<NLOperation<'a>>),

    LogicalAnd(Box<NLOperation<'a>>, Box<NLOperation<'a>>),
    LogicalOr(Box<NLOperation<'a>>, Box<NLOperation<'a>>),
    LogicalXor(Box<NLOperation<'a>>, Box<NLOperation<'a>>),

    BitAnd(Box<NLOperation<'a>>, Box<NLOperation<'a>>),
    BitOr(Box<NLOperation<'a>>, Box<NLOperation<'a>>),
    BitXor(Box<NLOperation<'a>>, Box<NLOperation<'a>>),

    ArithmeticNegate(Box<NLOperation<'a>>),
    BitNegate(Box<NLOperation<'a>>),

    BitLeftShift(Box<NLOperation<'a>>, Box<NLOperation<'a>>),
    BitRightShift(Box<NLOperation<'a>>, Box<NLOperation<'a>>),

    PropError(Box<NLOperation<'a>>),

    ArithmeticMod(Box<NLOperation<'a>>, Box<NLOperation<'a>>),
    ArithmeticAdd(Box<NLOperation<'a>>, Box<NLOperation<'a>>),
    ArithmeticSub(Box<NLOperation<'a>>, Box<NLOperation<'a>>),
    ArithmeticMul(Box<NLOperation<'a>>, Box<NLOperation<'a>>),
    ArithmeticDiv(Box<NLOperation<'a>>, Box<NLOperation<'a>>),
}

#[derive(PartialOrd, PartialEq, Debug)]
enum NLOperation<'a> {
    Block(NLBlock<'a>),
    Constant(OpConstant<'a>),
    Assign(OpAssignment<'a>),
    Tuple(Vec<NLOperation<'a>>),
    Operator(OpOperator<'a>),
}


pub struct NLFile<'a> {
    name: String,
    structs: Vec<NLStruct<'a>>,
    traits: Vec<NLTrait<'a>>,
    functions: Vec<NLFunction<'a>>,
}

impl<'a> NLFile<'a> {
    pub fn get_name(&self) -> &str { &self.name }
    pub fn get_structs(&self) -> &Vec<NLStruct> { &self.structs }
    pub fn get_traits(&self) -> &Vec<NLTrait> { &self.traits }
    pub fn get_functions(&self) -> &Vec<NLFunction> { &self.functions }
}

#[derive(Debug)]
pub struct ParseError {
    message: String,
}

impl std::error::Error for ParseError {
    fn description(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut Formatter) -> Result<(), std::fmt::Error> {
        write!(f, "{}", self.message)
    }
}

fn read_comment(input: &str) -> ParserResult<&str> {
    alt((
        preceded(tag("//"), terminated(take_until("\n"), tag("\n"))),
        preceded(tag("/*"), terminated(take_until("*/"), tag("*/"))),
    ))(input)
}

fn read_comments(input: &str) -> ParserResult<&str> {
    recognize(
        many0_count(terminated(read_comment, multispace0))
    )(input)
}

fn blank(input: &str) -> ParserResult<()> {
    value((), preceded(multispace0, read_comments))(input)
}

fn is_name(c: char) -> bool {
    match c {
        '_' => true,
        _ => (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z')
    }
}

fn read_struct_or_trait_name(input: &str) -> ParserResult<&str> {
    delimited(blank, alphanumeric1, blank)(input)
}

fn is_method_char(input: char) -> bool {
    match input {
        '_' => true,
        _ => is_alphanumeric(input as u8)
    }
}

fn read_method_name(input: &str) -> ParserResult<&str> {
    delimited(blank, take_while1(is_method_char), blank)(input)
}

fn read_tuple_of_variable_names(input: &str) -> ParserResult<Vec<&str>> {
    let (input, tuple_str) = delimited(char('('), take_while(|c| c != ')'), char(')'))(input)?;

    let (tuple_str, mut variables) = many0(terminated(read_variable_name, tuple((blank, char(','), blank))))(tuple_str)?;

    let (_, last_var) = opt(terminated(read_variable_name, blank))(tuple_str)?;
    match last_var {
        Some(var) => {
            variables.push(var);
        },
        _ => {} // Do nothing if there was no argument.
    }

    Ok((input, variables))
}

fn read_tuple(input: &str) -> ParserResult<NLOperation> {
    let (input, _) = blank(input)?;
    let (input, tuple_str) = delimited(char('('), take_while(|c| c != ')'), char(')'))(input)?;

    let (tuple_str, mut tuple) = many0(terminated(read_operation, tuple((blank, char(','), blank))))(tuple_str)?;

    let (_, last_item) = opt(terminated(read_operation, blank))(tuple_str)?;
    match last_item {
        Some(item) => {
            tuple.push(item);
        },
        _ => {} // Do nothing if there was no argument.
    }

    Ok((input, NLOperation::Tuple(tuple)))
}

fn read_single_variable(input: &str) -> ParserResult<Vec<&str>> {
    let (input, name) = read_variable_name(input)?;
    Ok((input, vec![name]))
}

fn read_boolean_constant(input: &str) -> ParserResult<OpConstant> {
    let (input, value) = alpha1(input)?;
    match value {
        "true" => Ok((input, OpConstant::Boolean(true))),
        "false" => Ok((input, OpConstant::Boolean(false))),
        _ => {
            let vek = VerboseErrorKind::Context("boolean must be true or false");

            let ve = VerboseError {
                errors: vec![(input, vek)]
            };

            Err(NomErr::Error(ve))
        },
    }
}

fn read_cast(input: &str) -> ParserResult<NLType> {
    let (input, _) = blank(input)?;
    let (input, _) = tag("as")(input)?;
    let (input, _) = blank(input)?;

    read_variable_type(input)
}

fn is_number(c: char) -> bool {
    match c {
        '.' => true,
        '-' => true,
        _ => c >= '0' && c <= '9'
    }
}

fn parse_integer<T>(input: &str) -> ParserResult<T>
    where T: std::str::FromStr {
    let value = input.parse::<T>();
    match value {
        Ok(value) => {
            // Its a valid integer.
            Ok((input, value))
        },
        _ => {
            let vek = VerboseErrorKind::Context("parse constant integer");

            let ve = VerboseError {
                errors: vec![(input, vek)]
            };

            Err(NomErr::Error(ve))
        }
    }
}

fn read_numerical_constant(input: &str) -> ParserResult<OpConstant> {
    let (input, number) = terminated(take_while1(is_number), blank)(input)?;
    let (input, cast) = opt(read_cast)(input)?;

    let cast = match cast {
        Some(cast) => cast,
        None => NLType::None,
    };

    if !number.contains(".") {
        let (_, value) = parse_integer::<i64>(number)?;
        Ok((input, OpConstant::Integer(value, cast)))
    } else {
        // Has to be a floating point number.
        let (_, value) = parse_integer::<f64>(number)?;
        Ok((input, OpConstant::Float(value, cast)))
    }
}

fn read_string_constant(input: &str) -> ParserResult<OpConstant> {
    // String constants are not pre-escaped. The escape can't be preformed without memory copying, and I want to compleatly avoid that in the
    // parsing phase.
    let (input, _) = blank(input)?;
    let (input, string) = delimited(char('"'), take_while(|c| c != '\"'), char('"'))(input)?;
    Ok((input, OpConstant::String(string)))
}

fn read_constant(input: &str) -> ParserResult<NLOperation> {
    let (input, constant) = alt((read_boolean_constant, read_numerical_constant, read_string_constant))(input)?;
    Ok((input, NLOperation::Constant(constant)))
}

fn read_assignment(input: &str) -> ParserResult<NLOperation> {

    // Are we defining?
    let (input, _) = blank(input)?;
    let (input, is_new) = opt(tag("let"))(input)?;
    let is_new = is_new.is_some();

    // What is our name?
    let (input, _) = blank(input)?;
    let (input, names) = alt((read_tuple_of_variable_names, read_single_variable))(input)?;

    let mut variables = Vec::new();
    variables.reserve(names.len());

    for name in names {
        let variable = OpVariable {
            name,
        };
        variables.push(variable);
    }

    // Are we given a type specification?
    let (input, _) = blank(input)?;
    let (input, has_type_assignment) = opt(char(':'))(input)?;
    let has_type_assignment = has_type_assignment.is_some();
    let (input, type_assignment) = if !has_type_assignment {
        (input, NLType::None)
    } else {
        read_variable_type(input)?
    };

    // Consume equal sign.
    let (input, _) = blank(input)?;
    let (input, _) = char('=')(input)?;
    let (input, _) = blank(input)?;

    // What's the value we are assigning to?
    let (input, _) = blank(input)?;
    let (input, assignment) = read_operation(input)?;

    let assignment = OpAssignment {
        is_new,
        to_assign: variables,
        type_assignment,
        assignment: Box::new(assignment),
    };

    Ok((input, NLOperation::Assign(assignment)))
}

/*
fn read_match_body(input: &str) -> ParserResult<NLMatchBody> {

}

fn read_value_match(input: &str) -> ParserResult<NLOperation> {
    unimplemented!()
}

fn read_type_match_first(input: &str) -> ParserResult<NLOperation> {
    unimplemented!()
}

fn read_type_match_many(input: &str) -> ParserResult<NLOperation> {
    unimplemented!()
}
*/

fn take_operator_symbol(input: &str) -> ParserResult<&str> {
    fn is_operator_symbol(c: char) -> bool {
        match c {
            '=' | '!' | '~' | '|' | '&' | '^' | '%' | '+' | '-' | '*' | '/' | '<' | '>' => true,
            _ => false,
        }
    }

    take_while1(is_operator_symbol)(input)
}

fn read_urinary_operator(input: &str) -> ParserResult<NLOperation> {
    let (input, _) = blank(input)?;
    let (input, operator) = take_operator_symbol(input)?;

    let (input, _) = blank(input)?;
    let (input, operand) = read_operation(input)?;
    let operand = Box::new(operand);

    match operator {
        "!" => {
            let operator = OpOperator::LogicalNegate(operand);
            Ok((input, NLOperation::Operator(operator)))
        },
        "~" => {
            let operator = OpOperator::BitNegate(operand);
            Ok((input, NLOperation::Operator(operator)))
        },
        "-" => {
            let operator = OpOperator::ArithmeticNegate(operand);
            Ok((input, NLOperation::Operator(operator)))
        },

        _ => {
            let vek = VerboseErrorKind::Context("unknown operator");

            let ve = VerboseError {
                errors: vec![(input, vek)]
            };

            Err(NomErr::Failure(ve))
        }
    }
}

fn read_binary_operator(input: &str) -> ParserResult<NLOperation> {
    let (input, _) = blank(input)?;
    let (input, operand_a) = read_sub_operation(input)?;
    let operand_a = Box::new(operand_a);

    let (input, _) = blank(input)?;
    let (input, operator) = take_operator_symbol(input)?;

    let (input, _) = blank(input)?;
    let (input, operand_b) = read_sub_operation(input)?;
    let operand_b = Box::new(operand_b);

    match operator {
        // Logical operators.
        "==" => {
            let operator = OpOperator::CompareEqual(operand_a, operand_b);
            Ok((input, NLOperation::Operator(operator)))
        },
        "!=" => {
            let operator = OpOperator::CompareNotEqual(operand_a, operand_b);
            Ok((input, NLOperation::Operator(operator)))
        },
        // TODO create formal errors for => and =< operators to help the noobs.
        ">=" => {
            let operator = OpOperator::CompareGreaterEqual(operand_a, operand_b);
            Ok((input, NLOperation::Operator(operator)))
        },
        "<=" => {
            let operator = OpOperator::CompareLessEqual(operand_a, operand_b);
            Ok((input, NLOperation::Operator(operator)))
        },

        ">" => {
            let operator = OpOperator::CompareGreater(operand_a, operand_b);
            Ok((input, NLOperation::Operator(operator)))
        },
        "<" => {
            let operator = OpOperator::CompareLess(operand_a, operand_b);
            Ok((input, NLOperation::Operator(operator)))
        },
        "&&" => {
            let operator = OpOperator::LogicalAnd(operand_a, operand_b);
            Ok((input, NLOperation::Operator(operator)))
        },
        "||" => {
            let operator = OpOperator::LogicalOr(operand_a, operand_b);
            Ok((input, NLOperation::Operator(operator)))
        },
        "^^" => {
            let operator = OpOperator::LogicalXor(operand_a, operand_b);
            Ok((input, NLOperation::Operator(operator)))
        },

        // Bitwise operators.
        "&" => {
            let operator = OpOperator::BitAnd(operand_a, operand_b);
            Ok((input, NLOperation::Operator(operator)))
        },
        "|" => {
            let operator = OpOperator::BitOr(operand_a, operand_b);
            Ok((input, NLOperation::Operator(operator)))
        },
        "^" => {
            let operator = OpOperator::BitXor(operand_a, operand_b);
            Ok((input, NLOperation::Operator(operator)))
        },
        "<<" => {
            let operator = OpOperator::BitLeftShift(operand_a, operand_b);
            Ok((input, NLOperation::Operator(operator)))
        },
        ">>" => {
            let operator = OpOperator::BitRightShift(operand_a, operand_b);
            Ok((input, NLOperation::Operator(operator)))
        },

        // Arithmetic operators.
        "+" => {
            let operator = OpOperator::ArithmeticAdd(operand_a, operand_b);
            Ok((input, NLOperation::Operator(operator)))
        },
        "-" => {
            let operator = OpOperator::ArithmeticSub(operand_a, operand_b);
            Ok((input, NLOperation::Operator(operator)))
        },
        "%" => {
            let operator = OpOperator::ArithmeticMod(operand_a, operand_b);
            Ok((input, NLOperation::Operator(operator)))
        },
        "/" => {
            let operator = OpOperator::ArithmeticDiv(operand_a, operand_b);
            Ok((input, NLOperation::Operator(operator)))
        },
        "*" => {
            let operator = OpOperator::ArithmeticMul(operand_a, operand_b);
            Ok((input, NLOperation::Operator(operator)))
        },

        _ => {
            let vek = VerboseErrorKind::Context("unknown operator");

            let ve = VerboseError {
                errors: vec![(input, vek)]
            };

            Err(NomErr::Failure(ve))
        }
    }
}

fn read_code_block(input: &str) -> ParserResult<NLOperation> {
    let (input, _) = blank(input)?;
    let (input, _) = char('{')(input)?;

    let (input, operations) = many0(read_operation)(input)?;

    let (input, _) = blank(input)?;
    let (input, _) = char('}')(input)?;

    Ok((input, NLOperation::Block(NLBlock {
        operations,
    })))
}

fn read_sub_operation(input: &str) -> ParserResult<NLOperation> {
    alt((read_code_block, read_tuple, read_assignment, read_constant, read_urinary_operator))(input)
}

fn read_operation(input: &str) -> ParserResult<NLOperation> {
    alt((read_code_block, read_tuple, read_assignment, read_binary_operator, read_constant, read_urinary_operator))(input)
}

fn read_argument_declaration(input: &str) -> ParserResult<NLArgument> {
    let (input, _) = blank(input)?;
    let (input, name) = opt(read_variable_name)(input)?;

    match name {
        Some(name) => {
            let (input, _) = blank(input)?;
            let (input, _) = char(':')(input)?;
            let (input, _) = blank(input)?;
            let (input, nl_type) = read_variable_type(input)?;
            let (input, _) = blank(input)?;

            let arg = NLArgument {
                name,
                nl_type
            };

            Ok((input, arg))
        },
        None => {

            let (post_input, is_ref) = opt(char('&'))(input)?;
            let is_ref = is_ref.is_some();

            if is_ref {
                let input = post_input;

                let (input, _) = blank(input)?;
                let (input, tagged) = opt(tag("self"))(input)?;
                if tagged.is_some() {
                    let arg = NLArgument {
                        name: "self",
                        nl_type: NLType::SelfReference,
                    };

                    return Ok((input, arg));
                }

                let (input, tagged) = opt(tag("mut"))(input)?;
                if tagged.is_some() {
                    let (input, _) = blank(input)?;
                    let (input, _) = tag("self")(input)?;

                    let arg = NLArgument {
                        name: "self",
                        nl_type: NLType::MutableSelfReference,
                    };

                    return Ok((input, arg));
                }
            }

            if !input.is_empty() {
                let vek = VerboseErrorKind::Context("could not read deceleration of argument correctly");

                let ve = VerboseError {
                    errors: vec![(input, vek)]
                };

                Err(NomErr::Failure(ve))
            } else {
                let vek = VerboseErrorKind::Context("there is no argument");

                let ve = VerboseError {
                    errors: vec![(input, vek)]
                };

                Err(NomErr::Error(ve))
            }
        },
    }
}

fn read_argument_deceleration_list(input: &str) -> ParserResult<Vec<NLArgument>> {
    let (input, arg_input) = delimited(char('('), take_while(|c| c != ')'), char(')'))(input)?;
    let (arg_input, mut arguments) = many0(terminated(read_argument_declaration, char(',')))(arg_input)?;

    let (_, last_arg) = opt(terminated(read_argument_declaration, blank))(arg_input)?;
    match last_arg {
        Some(arg) => {
            arguments.push(arg);
        },
        _ => {} // Do nothing if there was no argument.
    }

    Ok((input, arguments))
}

fn read_return_type(input: &str) -> ParserResult<NLType> {
    let (input, _) = blank(input)?;
    let (input, tagged) = opt(tag("->"))(input)?;

    if tagged.is_some() {
        let (input, _) = blank(input)?;
        let (input, nl_type) = read_variable_type(input)?;
        let (input, _) = blank(input)?;

        Ok((input, nl_type))
    } else {
        Ok((input, NLType::None))
    }
}

fn read_method(input: &str) -> ParserResult<NLImplementor> {
    let (input, _) = blank(input)?;
    let (input, _) = tag("met")(input)?;
    let (input, _) = blank(input)?;
    let (input, name) = read_method_name(input)?;
    let (input, _) = blank(input)?;
    let (input, args) = read_argument_deceleration_list(input)?;
    let (input, _) = blank(input)?;
    let (input, return_type) = read_return_type(input)?;
    let (input, _) = blank(input)?;
    let (input, block) = opt(read_code_block)(input)?;
    let block = match block {
        Some(block) => {
            match block {
                NLOperation::Block(block) => Some(block),
                _ => None,
            }
        },
        _ => None,
    };

    let method = NLFunction {
        name,
        arguments: args,
        return_type,
        block
    };

    // No block, we expect a semicolon.
    if method.block.is_none() {
        let (input, _) = char(';')(input)?;

        Ok((input, NLImplementor::Method(method)))
    } else {
        Ok((input, NLImplementor::Method(method)))
    }
}

fn read_function(input: &str) -> ParserResult<RootDeceleration> {
    let (input, _) = blank(input)?;
    let (input, _) = tag("fn")(input)?;
    let (input, _) = blank(input)?;
    let (input, name) = read_method_name(input)?;
    let (input, _) = blank(input)?;
    let (input, args) = read_argument_deceleration_list(input)?;
    let (input, _) = blank(input)?;
    let (input, return_type) = read_return_type(input)?;
    let (input, _) = blank(input)?;
    let (input, block) = opt(read_code_block)(input)?;
    let block = match block {
        Some(block) => {
            match block {
                NLOperation::Block(block) => Some(block),
                _ => None,
            }
        },
        _ => None,
    };

    let function = NLFunction {
        name,
        arguments: args,
        return_type,
        block
    };

    // No block, we expect a semicolon.
    if function.block.is_none() {
        let (input, _) = char(';')(input)?;

        Ok((input, RootDeceleration::Function(function)))
    } else {
        Ok((input, RootDeceleration::Function(function)))
    }
}

fn read_getter(input: &str) -> ParserResult<NLImplementor> {
    let (input, _) = blank(input)?;
    let (input, _) = tag("get")(input)?;
    let (input, name) = read_method_name(input)?;
    let (input, _) = blank(input)?;
    let (input, is_default) = opt(tuple((char(':'), blank, tag("default"), blank)))(input)?;

    if is_default.is_some() {
        let (input, nl_type) = read_return_type(input)?;
        let (input, _) = char(';')(input)?;

        let getter = NLGetter {
            name: String::from(name),
            args: vec![],
            nl_type,
            block: NLEncapsulationBlock::Default,
        };

        Ok((input, NLImplementor::Getter(getter)))
    } else {

        let (input, args) = read_argument_deceleration_list(input)?;
        let (input, nl_type) = read_return_type(input)?;
        let (input, block) = opt(read_code_block)(input)?;

        let block = match block {
            Some(block) => {
                match block {
                    NLOperation::Block(block) => Some(block),
                    _ => None,
                }
            },
            _ => None,
        };

        match block {
            Some(block) => {

                let getter = NLGetter {
                    name: String::from(name),
                    args,
                    nl_type,
                    block: NLEncapsulationBlock::Some(block),
                };

                Ok((input, NLImplementor::Getter(getter)))
            },
            None => {
                let (input, _) = char(';')(input)?;

                let getter = NLGetter {
                    name: String::from(name),
                    args,
                    nl_type,
                    block: NLEncapsulationBlock::None,
                };

                Ok((input, NLImplementor::Getter(getter)))
            }
        }
    }
}

fn read_setter(input: &str) -> ParserResult<NLImplementor> {
    let (input, _) = blank(input)?;
    let (input, _) = tag("set")(input)?;
    let (input, name) = read_method_name(input)?;
    let (input, _) = blank(input)?;
    let (input, is_default) = opt(tuple((char(':'), blank, tag("default"), blank, char(';'))))(input)?;

    if is_default.is_some() {
        let setter = NLSetter {
            name,
            args: vec![],
            block: NLEncapsulationBlock::Default
        };

        Ok((input, NLImplementor::Setter(setter)))
    } else  {

        let (input, args) = read_argument_deceleration_list(input)?;
        let (input, _) = blank(input)?;
        let (input, block) = opt(read_code_block)(input)?;
        let block = match block {
            Some(block) => {
                match block {
                    NLOperation::Block(block) => Some(block),
                    _ => None,
                }
            },
            _ => None,
        };

        match block {
            Some(block) => {
                let setter = NLSetter {
                    name,
                    args,
                    block: NLEncapsulationBlock::Some(block),
                };

                Ok((input, NLImplementor::Setter(setter)))
            },
            None => {
                let (input, _) = char(';')(input)?;

                let setter = NLSetter {
                    name,
                    args,
                    block: NLEncapsulationBlock::None,
                };

                Ok((input, NLImplementor::Setter(setter)))
            }
        }
    }
}

// TODO make it so you can specify required traits.
fn read_trait(input: &str) -> ParserResult<RootDeceleration> {
    let (input, _) = blank(input)?;
    let (input, _) = tag("trait")(input)?;
    let (input, _) = blank(input)?;
    let (input, name) = read_struct_or_trait_name(input)?;

    let (input, _) = blank(input)?;
    let (input, _) = char('{')(input)?;
    let (input, _) = blank(input)?;

    let (input, implementors) = many0(alt((read_method, read_getter, read_setter)))(input)?;

    let (input, _) = blank(input)?;
    let (input, _) = char('}')(input)?;

    let new_trait = NLTrait {
        name,
        implementors
    };

    Ok((input, RootDeceleration::Trait(new_trait)))
}

fn read_variable_name(input: &str) -> ParserResult<&str> {
    take_while1(is_name)(input)
}

fn identify_struct_or_trait_type(input: &str) -> ParserResult<NLType> {

    let (input, is_reference) = opt(char('&'))(input)?;
    let is_reference = is_reference.is_some();

    let (input, _) = blank(input)?;

    let (input, is_mutable) = if is_reference {
        let (input, is_mutable) = opt(tag("mut"))(input)?;
        let is_mutable = is_mutable.is_some();

        let (input, _) = blank(input)?;

        (input, is_mutable)
    } else {
        // If not a reference, this does not matter.
        (input, false)
    };

    let (input, is_struct) = opt(tag("dyn"))(input)?;
    let is_struct = is_struct.is_none();

    let (input, name) = read_struct_or_trait_name(input)?;

    if is_struct {
        // Its a struct.
        if is_reference {
            if is_mutable {
                Ok((input, NLType::MutableReferencedStruct(name)))
            } else {
                Ok((input, NLType::ReferencedStruct(name)))
            }
        } else {
            Ok((input, NLType::OwnedStruct(name)))
        }
    } else {
        // Its a trait.
        if is_reference {
            if is_mutable {
                Ok((input, NLType::MutableReferencedTrait(name)))
            } else {
                Ok((input, NLType::ReferencedTrait(name)))
            }
        } else {
            Ok((input, NLType::OwnedTrait(name)))
        }
    }
}

fn read_variable_type(input: &str) -> ParserResult<NLType> {
    let (input, _) = blank(input)?;
    let (input_new, type_name) = alphanumeric0(input)?;

    match type_name {
        "i8"   => Ok((input_new, NLType::I8)),
        "i16"  => Ok((input_new, NLType::I16)),
        "i32"  => Ok((input_new, NLType::I32)),
        "i64"  => Ok((input_new, NLType::I64)),
        "u8"   => Ok((input_new, NLType::U8)),
        "u16"  => Ok((input_new, NLType::U16)),
        "u32"  => Ok((input_new, NLType::U32)),
        "u64"  => Ok((input_new, NLType::U64)),
        "f32"  => Ok((input_new, NLType::F32)),
        "f64"  => Ok((input_new, NLType::F64)),
        "bool" => Ok((input_new, NLType::Boolean)),
        "str"  => Ok((input_new, NLType::OwnedString)),

        _ => {
            // Could it be a referenced string?
            let (input_new, _) = blank(input)?;
            let (input_new, is_referenced_string) = opt(preceded(blank, tag("str")))(input_new)?;
            let is_referenced_string = is_referenced_string.is_some();
            if is_referenced_string {
                return Ok((input_new, NLType::BorrowedString));
            } else {
                // Okay so we ether have Struct or Trait. Could even be a reference.
                return identify_struct_or_trait_type(input)
            }
        }
    }
}

fn read_struct_variable(input: &str) -> ParserResult<NLStructVariable> {

    let (input, _) = blank(input)?;
    let (input, name) = read_variable_name(input)?;

    let (input, _) = blank(input)?;
    let (input, _) = char(':')(input)?; // That : between the variable name and its type.
    let (input, _) = blank(input)?;
    let (input, nl_type) = read_variable_type(input)?;

    let var = NLStructVariable {
        name,
        my_type: nl_type,
    };

    Ok((input, var))
}

fn read_implementation(input: &str) -> ParserResult<NLImplementation> {
    let (input, _) = blank(input)?;
    let (input, _) = tag("impl")(input)?;
    let (input, name) = read_struct_or_trait_name(input)?;
    let (input, _) = char('{')(input)?;
    let (input, _) = blank(input)?;
    let (input, methods) = many0(alt((read_method, read_getter, read_setter)))(input)?;
    let (input, _) = blank(input)?;
    let (input, _) = char('}')(input)?;

    let implementation = NLImplementation {
        name,
        implementors: methods,
    };

    Ok((input, implementation))
}

fn read_struct(input: &str) -> ParserResult<RootDeceleration> {
    let (input, _) = blank(input)?;
    let (input, _) = tag("struct")(input)?;
    let (input, _) = blank(input)?;
    let (input, name) = read_struct_or_trait_name(input)?;
    let (input, _) = blank(input)?;
    let (input, _) = char('{')(input)?;
    let (input, _) = blank(input)?;
    let (input, mut variables) = many0(
        terminated(read_struct_variable, tuple((blank, char(','))))
    )(input)?;
    let (input, _) = blank(input)?;

    // Need to read the last struct.
    let (input, last_var) = opt(read_struct_variable)(input)?;
    match last_var {
        Some(var) => {
            variables.push(var);
        }
        _ => {} // Do nothing if we didn't have a last one.
    }

    let (input, _) = blank(input)?;
    let (input, _) = char('}')(input)?;
    let (input, implementations) = many0(read_implementation)(input)?;

    let nl_struct = NLStruct {
        name,
        variables,
        implementations
    };

    Ok((input, RootDeceleration::Struct(nl_struct)))
}

fn parse_file_root(input: &str) -> ParserResult<NLFile> {
    let mut file = NLFile {
        name: String::new(),
        structs: vec![],
        traits: vec![],
        functions: vec![],
    };

    if !input.is_empty() {
        let (input, root_defs) = many1(alt((read_struct, read_trait, read_function)))(input)?;

        for root_def in root_defs {
            match root_def {
                RootDeceleration::Struct(nl_struct) => {
                    file.structs.push(nl_struct);
                },
                RootDeceleration::Trait(nl_trait) => {
                    file.traits.push(nl_trait);
                },
                RootDeceleration::Function(nl_func) => {
                    file.functions.push(nl_func);
                },
            }
        }

        Ok((input, file))
    } else {
        Ok((input, file))
    }
}

pub fn parse_string<'a>(input: &'a str, file_name: &str) -> Result<NLFile<'a>, ParseError> {

    let file = parse_file_root(input);

    match file {
        Result::Err(err) => {
            match err {
                nom::Err::Error(e) | nom::Err::Failure(e) => {
                    let message = convert_error(input, e);

                    // Makes our error messages more readable when running tests.
                    #[cfg(test)]
                    println!("{}", message);

                    Err(ParseError {
                        message
                    })
                }
                nom::Err::Incomplete(_) => {
                    Err(ParseError {
                        message: "Unexpected end of file.".to_string()
                    })
                }
            }
        },
        Result::Ok(result) => {
            let (_, mut file) = result;

            file.name = file_name.to_string();

            Ok(file)
        }
    }
}

pub fn parse_file<T>(path: &Path, function: &dyn Fn(&NLFile) -> T) -> Result<T, Box<dyn std::error::Error>> {
    let mut input_file = File::open(&path)?;

    let mut contents = String::new();
    input_file.read_to_string(&mut contents)?;

    // This should *always* have a name since we shouldn't have been able to get to this point if it wasn't actually a file.
    let result = parse_string(&contents, &path.file_name().unwrap().to_str().unwrap());

    match result {
        Ok(result) => {
            Ok(function(&result))
        },
        Err(error) => {
            Err(Box::new(error))
        }
    }
}