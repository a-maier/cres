use nom::{
    bytes::complete::take_while1,
    character::complete::{i32, space1, u32},
    number::complete::double,
    sequence::preceded,
    IResult, Parser,
};

pub(crate) fn double_entry(line: &str) -> IResult<&str, f64> {
    preceded(space1, double).parse(line)
}

pub(crate) fn any_entry(line: &str) -> IResult<&str, &str> {
    preceded(space1, non_space).parse(line)
}

pub(crate) fn u32_entry(line: &str) -> IResult<&str, u32> {
    preceded(space1, u32).parse(line)
}

pub(crate) fn i32_entry(line: &str) -> IResult<&str, i32> {
    preceded(space1, i32).parse(line)
}

pub(crate) fn non_space(line: &str) -> IResult<&str, &str> {
    take_while1(|c: char| !c.is_ascii_whitespace()).parse(line)
}
