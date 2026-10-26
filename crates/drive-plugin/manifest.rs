//! Build-only parser. The manifest cannot execute code or refer to arbitrary UI widgets.
use crate::parameters::{
    MANIFEST_TYPE_ID, Parameter, SETTINGS_VERSION, STRIP_BINDINGS, Scale, diagram_binding,
    enable_binding, footer_power,
};
use std::collections::HashSet;
use toml_edit::{DocumentMut, Item, Table};

type Result<T> = std::result::Result<T, String>;
fn fail(message: &str) -> String {
    message.into()
}
fn keys(table: &Table, allowed: &[&str]) -> Result<()> {
    if table.iter().any(|(key, _)| !allowed.contains(&key)) {
        return Err(fail("unknown manifest field"));
    }
    Ok(())
}
fn string<'a>(table: &'a Table, key: &str) -> Result<&'a str> {
    table
        .get(key)
        .and_then(Item::as_str)
        .filter(|s| !s.is_empty() && !s.contains('\0'))
        .ok_or_else(|| format!("missing/invalid string: {key}"))
}
fn number(table: &Table, key: &str) -> Result<f64> {
    table
        .get(key)
        .and_then(|v| v.as_float().or_else(|| v.as_integer().map(|v| v as f64)))
        .filter(|v| v.is_finite())
        .ok_or_else(|| format!("missing/invalid number: {key}"))
}
fn class_id(text: &str) -> Result<[u8; 16]> {
    if text.len() != 32 || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(fail("class ID must have 32 hexadecimal digits"));
    }
    let mut bytes = [0; 16];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16)
            .map_err(|_| fail("invalid class ID"))?;
    }
    if bytes == [0; 16] {
        return Err(fail("zero class ID"));
    }
    Ok(bytes)
}

pub fn generate(input: &str) -> Result<String> {
    let document = input.parse::<DocumentMut>().map_err(|e| e.to_string())?;
    let root = document.as_table();
    keys(
        root,
        &[
            "schema_version",
            "settings_version",
            "state_version",
            "type_id",
            "vendor",
            "product",
            "version",
            "clap_id",
            "processor_id",
            "controller_id",
            "parameters",
            "strips",
        ],
    )?;
    for (key, expected) in [
        ("schema_version", 1),
        ("state_version", 1),
        ("settings_version", i64::from(SETTINGS_VERSION)),
    ] {
        if root.get(key).and_then(Item::as_integer) != Some(expected) {
            return Err(format!("unsupported {key}"));
        }
    }
    if string(root, "type_id")? != MANIFEST_TYPE_ID {
        return Err(fail("incorrect unit type ID"));
    }
    let vendor = string(root, "vendor")?;
    let product = string(root, "product")?;
    let version = string(root, "version")?;
    if [vendor, product]
        .iter()
        .any(|s| s.len() > 63 || s.chars().any(char::is_control))
    {
        return Err(fail("invalid descriptor name"));
    }
    if version != env!("CARGO_PKG_VERSION") {
        return Err(fail("plugin crate and manifest versions differ"));
    }
    let clap_id = string(root, "clap_id")?;
    if clap_id.len() > 255
        || !clap_id.contains('.')
        || !clap_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
    {
        return Err(fail("invalid CLAP ID"));
    }
    let processor = class_id(string(root, "processor_id")?)?;
    let controller = class_id(string(root, "controller_id")?)?;
    if processor == controller {
        return Err(fail("processor/controller IDs must differ"));
    }
    let parameters = root
        .get("parameters")
        .and_then(Item::as_array_of_tables)
        .ok_or_else(|| fail("missing parameters"))?;
    if parameters.len() != Parameter::ALL.len() {
        return Err(fail("expected seven parameters"));
    }
    let mut seen = HashSet::new();
    for parameter in parameters {
        keys(
            parameter,
            &["id", "key", "minimum", "maximum", "default", "scale"],
        )?;
        let id = parameter
            .get("id")
            .and_then(Item::as_integer)
            .and_then(|id| u32::try_from(id).ok())
            .ok_or_else(|| fail("invalid parameter ID"))?;
        let typed = Parameter::from_id(id).map_err(|e| e.to_string())?;
        if !seen.insert(id) {
            return Err(fail("duplicate parameter ID"));
        }
        let info = typed.info();
        let scale = match info.scale {
            Scale::Boolean => "boolean",
            Scale::Gain => "gain",
            Scale::Linear => "linear",
            Scale::Choice => "choice",
            Scale::Logarithmic => "logarithmic",
        };
        if string(parameter, "key")? != info.key
            || string(parameter, "scale")? != scale
            || number(parameter, "minimum")? != info.minimum
            || number(parameter, "maximum")? != info.maximum
            || (number(parameter, "default")? as f32) != info.default as f32
        {
            return Err(format!(
                "parameter contract differs from shared Drive settings: {}",
                info.key
            ));
        }
        typed
            .validate(number(parameter, "default")?)
            .map_err(|e| e.to_string())?;
    }
    let strips = root
        .get("strips")
        .and_then(Item::as_array_of_tables)
        .ok_or_else(|| fail("missing strips"))?;
    if strips.len() != STRIP_BINDINGS.len() {
        return Err(fail("expected two strips"));
    }
    let mut panels = String::new();
    for (index, strip) in strips.iter().enumerate() {
        keys(strip, &["footer", "controls", "power"])?;
        let footer = string(strip, "footer")?;
        if footer != STRIP_BINDINGS[index].0 {
            return Err(fail("incorrect strip footer/order"));
        }
        let controls = strip
            .get("controls")
            .and_then(Item::as_array)
            .ok_or_else(|| fail("missing panel controls"))?;
        if controls.len() != 3 {
            return Err(fail("expected three controls per strip"));
        }
        let power = footer_power(index);
        if strip.get("power").and_then(Item::as_str) != power.map(|p| p.info().key)
            || strip.contains_key("power") != power.is_some()
        {
            return Err(fail("incorrect footer power binding"));
        }
        panels.push_str(&format!(
            "Strip {{ footer: {footer:?}, power: {}, controls: [",
            power.map_or("None".into(), |p| format!("Some(Parameter::{p:?})"))
        ));
        for (slot, value) in controls.iter().enumerate() {
            let table = value
                .as_inline_table()
                .ok_or_else(|| fail("control must be an inline binding"))?;
            if table
                .iter()
                .any(|(key, _)| !["parameter", "control", "enabled_by", "diagram"].contains(&key))
            {
                return Err(fail("unknown control field"));
            }
            let text = |key: &str| table.get(key).and_then(toml_edit::Value::as_str);
            let (parameter, kind, variant) = STRIP_BINDINGS[index].1[slot];
            if text("parameter") != Some(parameter.info().key) || text("control") != Some(kind) {
                return Err(fail("missing, duplicate, or unknown panel binding/control"));
            }
            let enabled_parameter = enable_binding(parameter);
            let enable = enabled_parameter.map(|p| p.info().key);
            let diagram = diagram_binding(parameter);
            // Reject wrong types as well as wrong values for optional fields.
            if text("enabled_by") != enable
                || table.contains_key("enabled_by") != enable.is_some()
                || text("diagram") != diagram
                || table.contains_key("diagram") != diagram.is_some()
            {
                return Err(fail("unsupported enablement or diagram reference"));
            }
            panels.push_str(&format!("PanelControl {{ parameter: Parameter::{parameter:?}, control: Control::{variant}, enabled_by: {}, diagram: {diagram:?} }},", enabled_parameter.map_or("None".into(), |p| format!("Some(Parameter::{p:?})"))));
        }
        panels.push_str("] },");
    }
    let order = Parameter::ALL
        .map(|p| format!("Parameter::{p:?}"))
        .join(", ");
    Ok(format!(
        "pub const MANIFEST_SCHEMA_VERSION: u32 = 1;\npub const UNIT_TYPE_ID: &str = {:?};\npub const IDENTITY: Identity = Identity {{ vendor: {vendor:?}, product: {product:?}, version: {version:?}, clap_id: {clap_id:?}, processor_id: {processor:?}, controller_id: {controller:?} }};\npub const PANEL: [Strip; {strip_count}] = [{panels}];\npub const PARAMETER_ORDER: [Parameter; {parameter_count}] = [{order}];\n",
        MANIFEST_TYPE_ID,
        strip_count = STRIP_BINDINGS.len(),
        parameter_count = Parameter::COUNT
    ))
}
