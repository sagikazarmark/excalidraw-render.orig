use super::*;
const OPERATORS:&str="b b* B B* BDC BI BMC BT BX c cm CS cs d d0 d1 Do DP EI EMC ET EX f F f* G g gs h i ID J j K k l m M MP n q Q re RG rg ri s S SC sc SCN scn sh T* Tc Td TD Tf Tj TJ TL Tm Tr Ts Tw Tz v w W W* y ' \"";
pub(super) fn is_text(s: &str) -> bool {
    "BT ET Tc Tw Tz TL Tf Tr Ts Td TD Tm T* Tj TJ ' \""
        .split_whitespace()
        .any(|v| v == s)
}
pub(super) fn is_vector(s: &str) -> bool {
    "m l c v y h re S s f F f* B B* b b* n W W*"
        .split_whitespace()
        .any(|v| v == s)
}
fn latin(bytes: &[u8]) -> String {
    bytes.iter().map(|b| char::from(*b)).collect()
}
fn white(b: u8) -> bool {
    b"\0\t\n\x0c\r ".contains(&b)
}
fn delim(b: u8) -> bool {
    b"()<>[]{}/%".contains(&b)
}
fn tokenize(data: &[u8]) -> Vec<(bool, String)> {
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < data.len() {
        let b = data[i];
        if white(b) {
            i += 1;
            continue;
        }
        if b == b'%' {
            i = data[i..]
                .iter()
                .position(|b| *b == b'\n')
                .map(|p| i + p + 1)
                .unwrap_or(data.len());
            continue;
        }
        let start = i;
        if b == b'(' {
            i += 1;
            let mut depth = 1;
            while i < data.len() && depth > 0 {
                match data[i] {
                    b'\\' => {
                        i = (i + 2).min(data.len());
                        continue;
                    }
                    b'(' => depth += 1,
                    b')' => depth -= 1,
                    _ => {}
                }
                i += 1;
            }
            tokens.push((false, latin(&data[start..i])));
            continue;
        }
        if (b == b'<' || b == b'>') && data.get(i + 1) == Some(&b) {
            tokens.push((false, latin(&data[i..i + 2])));
            i += 2;
            continue;
        }
        if b == b'<' {
            i = data[i + 1..]
                .iter()
                .position(|b| *b == b'>')
                .map(|p| i + p + 2)
                .unwrap_or(data.len());
            tokens.push((false, latin(&data[start..i])));
            continue;
        }
        if b == b'/' {
            i += 1;
            while i < data.len() && !white(data[i]) && !delim(data[i]) {
                i += 1;
            }
            tokens.push((false, latin(&data[start..i])));
            continue;
        }
        if b"[]{} )>".contains(&b) {
            tokens.push((false, latin(&data[i..i + 1])));
            i += 1;
            continue;
        }
        while i < data.len() && !white(data[i]) && !delim(data[i]) {
            i += 1;
        }
        let token = latin(&data[start..i]);
        tokens.push((true, token.clone()));
        if token == "ID" {
            if i < data.len() && white(data[i]) {
                i += 1;
            }
            let start = i;
            let mut end = None;
            while i + 1 < data.len() {
                if &data[i..i + 2] == b"EI"
                    && (i == start || white(data[i - 1]))
                    && (i + 2 == data.len() || white(data[i + 2]))
                {
                    let mut e = i;
                    if e > start && white(data[e - 1]) {
                        e -= 1;
                    }
                    end = Some(e);
                    break;
                }
                i += 1;
            }
            let inline = if let Some(end) = end {
                let b = &data[start..end];
                i += 2;
                b
            } else {
                i = data.len();
                &data[start..]
            };
            tokens.push((false, format!("inline:{}:{}", inline.len(), sha256(inline))));
            if end.is_some() {
                tokens.push((true, "EI".into()));
            }
        }
    }
    tokens
}
pub(super) fn operations(
    objects: &Value,
    refs: &[Value],
    resources: &Value,
    errors: &mut Vec<String>,
) -> Result<Vec<Value>> {
    struct Walk<'a> {
        objects: &'a Value,
        result: Vec<Value>,
        errors: &'a mut Vec<String>,
    }
    impl Walk<'_> {
        fn walk(
            &mut self,
            refs: &[Value],
            resources: &Value,
            scope: &str,
            stack: &[Value],
        ) -> Result<()> {
            let xobjects = resolve_value(self.objects, &get(resources, "/XObject", json!({})));
            let states = resolve_value(self.objects, &get(resources, "/ExtGState", json!({})));
            for r in refs {
                let Some(stream) = stream_value(self.objects, r)? else {
                    continue;
                };
                let mut operands: Vec<String> = vec![];
                for (word, token) in tokenize(&stream) {
                    if word && OPERATORS.split_whitespace().any(|s| s == token) {
                        self.result.push(json!({"operands":operands,"operator":token,"scope":scope,"sequence":self.result.len()}));
                        if let Some(resource) = operands.last() {
                            let mut form_ref = Value::Null;
                            let mut soft = false;
                            if token == "Do" {
                                form_ref = xobjects[resource].clone();
                            } else if token == "gs" {
                                let state = resolve(self.objects, &states[resource]);
                                form_ref =
                                    resolve_value(self.objects, &state["/SMask"])["/G"].clone();
                                soft = true;
                            }
                            let form = stream_dict(self.objects, &form_ref);
                            if form["/Subtype"] == "/Form" {
                                let what = if soft {
                                    "soft-mask Form"
                                } else {
                                    "Form XObject"
                                };
                                if stack.contains(&form_ref) {
                                    self.errors.push(format!(
                                        "recursive {what} {} in {scope}",
                                        form_ref.as_str().unwrap_or("")
                                    ));
                                } else if stack.len() >= 32 {
                                    self.errors
                                        .push(format!("{what} depth exceeds 32 in {scope}"));
                                } else {
                                    let default = if soft { json!({}) } else { resources.clone() };
                                    let nested = resolve_value(
                                        self.objects,
                                        &get(&form, "/Resources", default),
                                    );
                                    let mut next = stack.to_vec();
                                    next.push(form_ref.clone());
                                    self.walk(
                                        &[form_ref],
                                        &nested,
                                        &format!(
                                            "{scope}{resource}{}",
                                            if soft { "/SMask" } else { "" }
                                        ),
                                        &next,
                                    )?;
                                }
                            }
                        }
                        operands.clear();
                    } else {
                        operands.push(token);
                    }
                }
            }
            Ok(())
        }
    }
    let mut walk = Walk {
        objects,
        result: vec![],
        errors,
    };
    walk.walk(refs, resources, "page", &[])?;
    Ok(walk.result)
}
