use super::{MirType, ScalarType, StateTokenKind};

pub fn render_mir_type(ty: &MirType) -> String {
    match ty {
        MirType::Scalar(scalar) => format!("scalar:{}", scalar_name(*scalar)),
        MirType::Data(name) => format!("data:{}", escape_token(name)),
        MirType::LayoutData(name) => format!("layout:{}", escape_token(name)),
        MirType::Class(name) => format!("class:{}", escape_token(name)),
        MirType::UniqueClass(name) => format!("unique:{}", escape_token(name)),
        MirType::Interface(name) => format!("interface:{}", escape_token(name)),
        MirType::Error(name) => format!("error:{}", escape_token(name)),
        MirType::Image(name) => format!("image:{}", escape_token(name)),
        MirType::HostImage(name) => format!("host_image:{}", escape_token(name)),
        MirType::Capability { class_name, path } => {
            format!(
                "capability:class={},path={}",
                escape_token(class_name),
                escape_token(path)
            )
        }
        MirType::Table { item, rows } => {
            format!("table:item={},rows={}", escape_token(item), rows)
        }
        MirType::Column { item, table, rows } => {
            format!(
                "column:item={},table={},rows={}",
                escape_token(item),
                escape_token(table),
                rows
            )
        }
        MirType::Mask { table, rows } => {
            format!("mask:table={},rows={}", escape_token(table), rows)
        }
        MirType::RowToken { table } => format!("row_token:table={}", escape_token(table)),
        MirType::StateToken(kind) => format!("state:{}", state_token_name(kind)),
        MirType::CapacityToken { owner } => {
            format!("capacity:owner={}", escape_token(owner))
        }
        MirType::Never => "never".to_string(),
        MirType::Unknown => "unknown".to_string(),
    }
}

pub fn parse_mir_type(text: &str) -> Result<MirType, String> {
    let trimmed = text.trim();
    if trimmed == "never" {
        return Ok(MirType::Never);
    }
    if trimmed == "unknown" {
        return Ok(MirType::Unknown);
    }
    let Some((kind, payload)) = trimmed.split_once(':') else {
        return Err(format!("bad type `{text}`"));
    };
    match kind {
        "scalar" => parse_scalar(payload).map(MirType::Scalar),
        "data" => Ok(MirType::Data(unescape_token(payload))),
        "layout" => Ok(MirType::LayoutData(unescape_token(payload))),
        "class" => Ok(MirType::Class(unescape_token(payload))),
        "unique" => Ok(MirType::UniqueClass(unescape_token(payload))),
        "interface" => Ok(MirType::Interface(unescape_token(payload))),
        "error" => Ok(MirType::Error(unescape_token(payload))),
        "image" => Ok(MirType::Image(unescape_token(payload))),
        "host_image" => Ok(MirType::HostImage(unescape_token(payload))),
        "capability" => {
            let class_name = parse_attr(payload, "class=")?;
            let path = parse_attr(payload, "path=")?;
            Ok(MirType::Capability { class_name, path })
        }
        "table" => {
            let item = parse_attr(payload, "item=")?;
            let rows = parse_attr(payload, "rows=")?
                .parse::<u64>()
                .map_err(|_| format!("bad table rows in `{text}`"))?;
            Ok(MirType::Table { item, rows })
        }
        "column" => {
            let item = parse_attr(payload, "item=")?;
            let table = parse_attr(payload, "table=")?;
            let rows = parse_attr(payload, "rows=")?
                .parse::<u64>()
                .map_err(|_| format!("bad column rows in `{text}`"))?;
            Ok(MirType::Column { item, table, rows })
        }
        "mask" => {
            let table = parse_attr(payload, "table=")?;
            let rows = parse_attr(payload, "rows=")?
                .parse::<u64>()
                .map_err(|_| format!("bad mask rows in `{text}`"))?;
            Ok(MirType::Mask { table, rows })
        }
        "row_token" => {
            let table = parse_attr(payload, "table=")?;
            Ok(MirType::RowToken { table })
        }
        "state" => parse_state_token(payload).map(MirType::StateToken),
        "capacity" => {
            let owner = parse_attr(payload, "owner=")?;
            Ok(MirType::CapacityToken { owner })
        }
        _ => Err(format!("unsupported type kind `{kind}`")),
    }
}

fn scalar_name(scalar: ScalarType) -> &'static str {
    match scalar {
        ScalarType::Bool => "Bool",
        ScalarType::I64 => "I64",
        ScalarType::U32 => "U32",
        ScalarType::U64 => "U64",
        ScalarType::String => "String",
        ScalarType::None => "None",
    }
}

fn parse_scalar(text: &str) -> Result<ScalarType, String> {
    match text {
        "Bool" => Ok(ScalarType::Bool),
        "I64" => Ok(ScalarType::I64),
        "U32" => Ok(ScalarType::U32),
        "U64" => Ok(ScalarType::U64),
        "String" => Ok(ScalarType::String),
        "None" => Ok(ScalarType::None),
        _ => Err(format!("unsupported scalar `{text}`")),
    }
}

fn state_token_name(kind: &StateTokenKind) -> String {
    match kind {
        StateTokenKind::Table(name) => format!("table:{}", escape_token(name)),
        StateTokenKind::Mmio(name) => format!("mmio:{}", escape_token(name)),
        StateTokenKind::Atomic(name) => format!("atomic:{}", escape_token(name)),
        StateTokenKind::Sync(name) => format!("sync:{}", escape_token(name)),
    }
}

fn parse_state_token(text: &str) -> Result<StateTokenKind, String> {
    let Some((kind, name)) = text.split_once(':') else {
        return Err(format!("bad state token `{text}`"));
    };
    let name = unescape_token(name);
    match kind {
        "table" => Ok(StateTokenKind::Table(name)),
        "mmio" => Ok(StateTokenKind::Mmio(name)),
        "atomic" => Ok(StateTokenKind::Atomic(name)),
        "sync" => Ok(StateTokenKind::Sync(name)),
        _ => Err(format!("bad state token kind `{kind}`")),
    }
}

fn parse_attr(payload: &str, key: &str) -> Result<String, String> {
    payload
        .split(',')
        .find_map(|part| part.strip_prefix(key))
        .map(unescape_token)
        .ok_or_else(|| format!("missing `{key}` in `{payload}`"))
}

pub fn escape_token(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        match ch {
            ',' | '=' | '%' => {
                out.push('%');
                out.push_str(&format!("{:02X}", ch as u32));
            }
            _ => out.push(ch),
        }
    }
    out
}

pub fn unescape_token(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '%' {
            let hi = chars.next();
            let lo = chars.next();
            if let (Some(h), Some(l)) = (hi, lo) {
                let hex = format!("{h}{l}");
                if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                    out.push(byte as char);
                    continue;
                }
            }
            out.push('%');
            if let Some(h) = hi {
                out.push(h);
            }
            if let Some(l) = lo {
                out.push(l);
            }
        } else {
            out.push(ch);
        }
    }
    out
}

pub fn parse_quoted_or_token(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
        unescape_token(&trimmed[1..trimmed.len() - 1])
    } else {
        unescape_token(trimmed)
    }
}

pub fn render_quoted_if_needed(text: &str) -> String {
    if text
        .chars()
        .any(|ch| ch.is_whitespace() || ch == ',' || ch == '=')
    {
        format!("\"{}\"", escape_token(text))
    } else {
        escape_token(text)
    }
}
