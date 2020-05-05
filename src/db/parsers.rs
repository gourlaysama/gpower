use super::*;
use anyhow::*;
use nom::{bytes::complete::*, combinator::*, multi::*, sequence::*, IResult};
use std::collections::HashMap;

fn content(i: &str) -> IResult<&str, &str> {
    take_while(|c: char| c != '\r' && c != '\n')(i)
}

fn line_ending(i: &str) -> IResult<&str, &str> {
    take_while1(|c: char| c == '\r' || c == '\n')(i)
}

fn comment(i: &str) -> IResult<&str, ()> {
    let (i, _) = many0(tuple((tag("#"), &content, &line_ending)))(i)?;

    Ok((i, ()))
}

fn name(input: &str) -> IResult<&str, (u16, &str)> {
    let space = take_while1(|c: char| c.is_whitespace());
    let digits = take_while(|c: char| c.is_digit(16));
    let hex_value = map_res(digits, |s| u16::from_str_radix(s, 16));

    let (input, (id, _, name)) = tuple((hex_value, space, content))(input)?;

    Ok((input, (id, name)))
}

fn parse_subentry(input: &str) -> IResult<&str, (u16, &str)> {
    let tab = tag("\t");

    let (input, (_, _, (device_id, device_name), _)) =
        tuple((comment, &tab, name, &line_ending))(input)?;

    let interface = tuple((comment, &tab, &tab, content, &line_ending));
    // drop interfaces
    let (input, _) = many0(interface)(input)?;

    Ok((input, (device_id, device_name)))
}

fn parse_vendor(input: &str) -> IResult<&str, Result<Vendor, DeviceClass>> {
    let mut map = HashMap::new();
    let (input, (_, class, (vendor_id, vendor_name), _)) = tuple((
        comment,
        opt(tuple((tag("C"), take_while1(|c: char| c.is_whitespace())))),
        name,
        &line_ending,
    ))(input)?;

    let (input, devices) = many0(parse_subentry)(input)?;

    for d in devices {
        map.insert(d.0, d.1.to_string());
    }

    if class.is_none() {
        Ok((
            input,
            Ok(Vendor {
                id: vendor_id,
                name: vendor_name.to_string(),
                devices: map,
            }),
        ))
    } else {
        Ok((
            input,
            Err(DeviceClass {
                id: vendor_id,
                name: vendor_name.to_string(),
                subclasses: map,
            }),
        ))
    }
}

fn parse_file(input: &str) -> IResult<&str, Db> {
    let mut vendor_map = HashMap::with_capacity(20000);
    let mut class_map = HashMap::with_capacity(50);

    let (input, vendors) = many0(parse_vendor)(input)?;

    for v in vendors {
        match v {
            Ok(vendor) => {
                vendor_map.insert(vendor.id, vendor);
            }
            Err(class) => {
                class_map.insert(class.id, class);
            }
        };
    }

    Ok((
        input,
        Db {
            vendors: vendor_map,
            classes: class_map,
        },
    ))
}

pub fn parse_all(input: &str) -> Result<Db> {
    let (_, db) = parse_file(input).map_err(|e| anyhow!("failed to parse file {}", e))?;

    Ok(db)
}
