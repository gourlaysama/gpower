use anyhow::*;
use nom::{bytes::complete::*, combinator::*, multi::*, sequence::*, IResult};
use std::collections::HashMap;

fn parse_name(input: &str) -> IResult<&str, (u32, &str)> {
    let space = take_while1(|c: char| c.is_whitespace());
    let content = take_while1(|c: char| c != '\r' && c != '\n');
    let digits = take_while(|c: char| c.is_digit(16));
    let hex_value = map_res(digits, |s| u32::from_str_radix(s, 16));

    let (input, (id, _, name)) = tuple((hex_value, space, content))(input)?;

    Ok((input, (id, name)))
}

fn parse_device(input: &str) -> IResult<&str, (u32, &str)> {
    let tab = tag("\t");
    let line_ending = take_while1(|c: char| c == '\r' || c == '\n');
    let content = take_while1(|c: char| c != '\r' && c != '\n');
    let content0 = take_while(|c: char| c != '\r' && c != '\n');
    let comment = many0(tuple((tag("#"), &content0, &line_ending)));

    let (input, (_, _, (device_id, device_name), _)) =
        tuple((comment, &tab, parse_name, &line_ending))(input)?;

    let interface = tuple((&tab, &tab, &content, &line_ending));
    // drop interfaces
    let (input, _) = many0(interface)(input)?;

    Ok((input, (device_id, device_name)))
}

pub struct Vendor {
    pub id: u32,
    pub name: String,
    pub devices: HashMap<u32, String>,
}

fn parse_vendor(input: &str) -> IResult<&str, Vendor> {
    let mut map = HashMap::new();
    let line_ending = take_while1(|c: char| c == '\r' || c == '\n');
    let content0 = take_while(|c: char| c != '\r' && c != '\n');
    let comment = many0(tuple((tag("#"), &content0, &line_ending)));

    let (input, (_, (vendor_id, vendor_name), _)) =
        tuple((&comment, parse_name, &line_ending))(input)?;

    let (input, devices) = many0(parse_device)(input)?;

    for d in devices {
        map.insert(d.0, d.1.to_string());
    }

    Ok((
        input,
        Vendor {
            id: vendor_id,
            name: vendor_name.to_string(),
            devices: map,
        },
    ))
}

fn parse_file(input: &str) -> IResult<&str, HashMap<u32, Vendor>> {
    let mut map = HashMap::with_capacity(20000);

    let (input, vendors) = many0(parse_vendor)(input)?;

    for v in vendors {
        map.insert(v.id, v);
    }

    Ok((input, map))
}

pub fn parse_all(input: &str) -> Result<HashMap<u32, Vendor>> {
    let (_, vendors) = parse_file(input).map_err(|e| anyhow!("failed to parse file {}", e))?;

    Ok(vendors)
}
