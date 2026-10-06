//! Small strict JSON parser, avoiding dependencies for offline builds.
use crate::board::Position;
use std::collections::BTreeMap;
#[derive(Clone, Debug)]
pub enum Json {
    Null,
    Bool(bool),
    Num(i64),
    Real(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(BTreeMap<String, Json>),
}
impl Json {
    pub fn get(&self, k: &str) -> Result<&Self, String> {
        if let Self::Obj(o) = self {
            o.get(k).ok_or_else(|| format!("Missing {k}"))
        } else {
            Err("Expected object".into())
        }
    }
    pub fn num(&self) -> Result<i64, String> {
        if let Self::Num(v) = self {
            Ok(*v)
        } else {
            Err("Expected integer".into())
        }
    }
    pub fn str(&self) -> Result<&str, String> {
        if let Self::Str(v) = self {
            Ok(v)
        } else {
            Err("Expected string".into())
        }
    }
    pub fn arr(&self) -> Result<&[Self], String> {
        if let Self::Arr(v) = self {
            Ok(v)
        } else {
            Err("Expected array".into())
        }
    }
    pub fn boolean(&self) -> Result<bool, String> {
        if let Self::Bool(v) = self {
            Ok(*v)
        } else {
            Err("Expected boolean".into())
        }
    }
}
pub fn quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c < ' ' => out.push_str(&format!("\\u{:04x}", c as u32)),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}
pub fn parse(s: &str) -> Result<Json, String> {
    struct P<'a> {
        s: &'a [u8],
        i: usize,
    }
    impl P<'_> {
        fn ws(&mut self) {
            while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
                self.i += 1
            }
        }
        fn val(&mut self, depth: u32) -> Result<Json, String> {
            if depth > 128 {
                return Err("JSON too deep".into());
            }
            self.ws();
            let c = *self.s.get(self.i).ok_or("Unexpected end")?;
            self.i += 1;
            Ok(match c {
                b'n' => {
                    self.word(b"ull")?;
                    Json::Null
                }
                b't' => {
                    self.word(b"rue")?;
                    Json::Bool(true)
                }
                b'f' => {
                    self.word(b"alse")?;
                    Json::Bool(false)
                }
                b'"' => {
                    let mut out = String::new();
                    let mut start = self.i;
                    loop {
                        let c = *self.s.get(self.i).ok_or("Unclosed string")?;
                        if c == b'"' || c == b'\\' {
                            out.push_str(
                                std::str::from_utf8(&self.s[start..self.i]).map_err(|_| "UTF8")?,
                            );
                            self.i += 1;
                            if c == b'"' {
                                break;
                            }
                            let e = *self.s.get(self.i).ok_or("Escape")?;
                            self.i += 1;
                            out.push(match e {
                                b'"' => '"',
                                b'\\' => '\\',
                                b'/' => '/',
                                b'n' => '\n',
                                b'r' => '\r',
                                b't' => '\t',
                                b'b' => '\x08',
                                b'f' => '\x0c',
                                b'u' => {
                                    let end = self.i + 4;
                                    let h = self.s.get(self.i..end).ok_or("Unicode")?;
                                    self.i = end;
                                    char::from_u32(
                                        u32::from_str_radix(
                                            std::str::from_utf8(h).map_err(|_| "Unicode")?,
                                            16,
                                        )
                                        .map_err(|_| "Unicode")?,
                                    )
                                    .ok_or("Unicode")?
                                }
                                _ => return Err("Escape".into()),
                            });
                            start = self.i;
                        } else {
                            if c < 32 {
                                return Err("Control character".into());
                            }
                            self.i += 1;
                        }
                    }
                    Json::Str(out)
                }
                b'[' => {
                    let mut a = vec![];
                    self.ws();
                    if self.s.get(self.i) == Some(&b']') {
                        self.i += 1
                    } else {
                        loop {
                            a.push(self.val(depth + 1)?);
                            self.ws();
                            let c = self.s.get(self.i);
                            self.i += 1;
                            if c == Some(&b']') {
                                break;
                            }
                            if c != Some(&b',') {
                                return Err("Array separator".into());
                            }
                        }
                    }
                    Json::Arr(a)
                }
                b'{' => {
                    let mut o = BTreeMap::new();
                    self.ws();
                    if self.s.get(self.i) == Some(&b'}') {
                        self.i += 1
                    } else {
                        loop {
                            let k = self.val(depth + 1)?.str()?.to_owned();
                            self.ws();
                            if self.s.get(self.i) != Some(&b':') {
                                return Err("Expected colon".into());
                            }
                            self.i += 1;
                            let v = self.val(depth + 1)?;
                            if o.insert(k, v).is_some() {
                                return Err("Duplicate key".into());
                            }
                            self.ws();
                            let c = self.s.get(self.i);
                            self.i += 1;
                            if c == Some(&b'}') {
                                break;
                            }
                            if c != Some(&b',') {
                                return Err("Object separator".into());
                            }
                        }
                    }
                    Json::Obj(o)
                }
                b'-' | b'0'..=b'9' => {
                    let start = self.i - 1;
                    while self.s.get(self.i).is_some_and(u8::is_ascii_digit) {
                        self.i += 1
                    }
                    let t = std::str::from_utf8(&self.s[start..self.i]).unwrap();
                    let digits = t.trim_start_matches('-');
                    if digits.len() > 1 && digits.starts_with('0') {
                        return Err("Leading zero".into());
                    }
                    if self
                        .s
                        .get(self.i)
                        .is_some_and(|c| *c == b'.' || *c == b'e' || *c == b'E')
                    {
                        if self.s.get(self.i) == Some(&b'.') {
                            self.i += 1;
                            let n = self.i;
                            while self.s.get(self.i).is_some_and(u8::is_ascii_digit) {
                                self.i += 1
                            }
                            if n == self.i {
                                return Err("Missing fraction".into());
                            }
                        }
                        if self.s.get(self.i).is_some_and(|c| *c == b'e' || *c == b'E') {
                            self.i += 1;
                            if self.s.get(self.i).is_some_and(|c| *c == b'+' || *c == b'-') {
                                self.i += 1
                            }
                            let n = self.i;
                            while self.s.get(self.i).is_some_and(u8::is_ascii_digit) {
                                self.i += 1
                            }
                            if n == self.i {
                                return Err("Missing exponent".into());
                            }
                        }
                        let n: f64 = std::str::from_utf8(&self.s[start..self.i])
                            .unwrap()
                            .parse()
                            .map_err(|_| "Invalid number")?;
                        if !n.is_finite() {
                            return Err("Number overflow".into());
                        }
                        Json::Real(n)
                    } else {
                        Json::Num(t.parse().map_err(|_| "Invalid integer")?)
                    }
                }
                _ => return Err("Invalid JSON".into()),
            })
        }
        fn word(&mut self, w: &[u8]) -> Result<(), String> {
            if self.s.get(self.i..self.i + w.len()) != Some(w) {
                return Err("Invalid literal".into());
            }
            self.i += w.len();
            Ok(())
        }
    }
    let mut p = P {
        s: s.as_bytes(),
        i: 0,
    };
    let v = p.val(0)?;
    p.ws();
    if p.i != p.s.len() {
        return Err("Trailing JSON".into());
    }
    Ok(v)
}
pub fn position(v: &Json) -> Result<Position, String> {
    if let Json::Str(s) = v {
        return Position::sequence(s);
    }
    let board = v.get("board")?.arr()?;
    let shape = v.get("shape")?.arr()?;
    if board.len() != 8 || shape.len() != 8 {
        return Err("Expected 8 rows".into());
    }
    let mut p = Position {
        side: v
            .get("turn")
            .map_or(Ok(1), Json::num)?
            .try_into()
            .map_err(|_| "turn")?,
        ..Position::default()
    };
    let mut labels = vec![String::new(); 64];
    for r in 0..8 {
        let b = board[r].arr()?;
        let s = shape[r].arr()?;
        if b.len() != 8 || s.len() != 8 {
            return Err("Expected 8 columns".into());
        }
        for c in 0..8 {
            let i = r * 8 + c;
            let n = b[c].num()?;
            let label = match &s[c] {
                Json::Null => "",
                x => x.str()?,
            };
            labels[i] = label.into();
            match n {
                1 => p.black |= 1 << i,
                -1 => p.white |= 1 << i,
                0 => {
                    if !label.is_empty() {
                        return Err("Shape on empty cell".into());
                    }
                    continue;
                }
                _ => return Err("Invalid cell".into()),
            }
            match label {
                "h-left" => p.hleft |= 1 << i,
                "v-top" => p.vtop |= 1 << i,
                "h-right" | "v-bottom" => (),
                _ => return Err("Missing shape".into()),
            }
        }
    }
    p.validate()?;
    for i in 0..64 {
        if (p.black | p.white) & (1 << i) == 0 {
            continue;
        }
        let expected = if p.hleft & (1 << i) != 0 {
            "h-left"
        } else if (p.hleft << 1) & (1 << i) != 0 {
            "h-right"
        } else if p.vtop & (1 << i) != 0 {
            "v-top"
        } else {
            "v-bottom"
        };
        if labels[i] != expected {
            return Err("Mismatched shape".into());
        }
    }
    Ok(p)
}
pub fn position_json(p: Position) -> String {
    let rows = (0..8)
        .map(|r| {
            format!(
                "[{}]",
                (0..8)
                    .map(|c| {
                        let bit = 1u64 << (r * 8 + c);
                        if p.black & bit != 0 {
                            "1"
                        } else if p.white & bit != 0 {
                            "-1"
                        } else {
                            "0"
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(",")
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let shapes = (0..8)
        .map(|r| {
            format!(
                "[{}]",
                (0..8)
                    .map(|c| {
                        let bit = 1u64 << (r * 8 + c);
                        quote(if p.hleft & bit != 0 {
                            "h-left"
                        } else if (p.hleft << 1) & bit != 0 {
                            "h-right"
                        } else if p.vtop & bit != 0 {
                            "v-top"
                        } else if (p.vtop << 8) & bit != 0 {
                            "v-bottom"
                        } else {
                            ""
                        })
                    })
                    .collect::<Vec<_>>()
                    .join(",")
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"board\":[{rows}],\"shape\":[{shapes}],\"turn\":{}}}",
        p.side
    )
}
