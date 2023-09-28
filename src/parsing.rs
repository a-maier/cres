use nom::{sequence::preceded, character::complete::{i32, space1, u32}, number::complete::double, IResult, bytes::complete::take_while1};

pub(crate) fn double_entry(line: &str) -> IResult<&str, f64> {
    preceded(space1, double)(line)
}

pub(crate) fn any_entry(line: &str) -> IResult<&str, &str> {
    preceded(space1, non_space)(line)
}

pub(crate) fn u32_entry(line: &str) -> IResult<&str, u32> {
    preceded(space1, u32)(line)
}

pub(crate) fn i32_entry(line: &str) -> IResult<&str, i32> {
    preceded(space1, i32)(line)
}

pub(crate) fn non_space(line: &str) -> IResult<&str, &str> {
    take_while1(|c: char| !c.is_ascii_whitespace())(line)
}
