use super::{
    enter::Entered,
    item_move::{Coord, Request},
    msgbox,
    panels::Panels,
};
use formats::{
    data::item_effect::stack_size,
    net::{account::Item, client_config::Config},
};

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct Cost {
    pub item: i32,
    pub amount: i32,
    pub per_unit: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct Rule {
    pub materials: Vec<Cost>,
    pub gold: i32,
    pub gold_per_unit: bool,
    pub effect: i32,
    pub value: i32,
}

fn decimal(text: &mut &[u8]) -> Option<i32> {
    while text.first().is_some_and(u8::is_ascii_whitespace) {
        *text = &text[1..];
    }

    let negative = text.first() == Some(&b'-');

    if matches!(text.first(), Some(b'+' | b'-')) {
        *text = &text[1..];
    }

    let mut value = 0i32;

    let mut digits = 0;

    while let Some(&byte @ b'0'..=b'9') = text.first() {
        value = value.wrapping_mul(10).wrapping_add(i32::from(byte - b'0'));

        *text = &text[1..];

        digits += 1;
    }

    (digits != 0).then_some(if negative {
        value.wrapping_neg()
    } else {
        value
    })
}

fn colon(text: &mut &[u8]) -> bool {
    if text.first() != Some(&b':') {
        return false;
    }

    *text = &text[1..];

    true
}

pub fn rule(config: &Config, item: u16) -> Option<Rule> {
    let count = config.integer("agrup.n");

    if !(1..=16).contains(&count) {
        return None;
    }

    for i in 1..=count {
        if !config
            .get(&format!("agrup.i{i}"))
            .unwrap_or("")
            .split(',')
            .any(|id| decimal(&mut id.as_bytes()) == Some(i32::from(item)))
        {
            continue;
        }

        let materials = config
            .get(&format!("agrup.c{i}"))
            .unwrap_or("")
            .split(',')
            .filter_map(|text| {
                let mut text = text.as_bytes();

                let item = decimal(&mut text).filter(|&v| v > 0)?;

                if !colon(&mut text) {
                    return None;
                }

                let amount = decimal(&mut text).filter(|&v| v > 0)?;

                let per_unit = colon(&mut text) && text.first() == Some(&b'u');

                Some(Cost {
                    item,
                    amount,
                    per_unit,
                })
            })
            .take(6)
            .collect::<Vec<_>>();

        let mut gold = config.get(&format!("agrup.g{i}")).unwrap_or("").as_bytes();

        let amount = decimal(&mut gold).unwrap_or(0).max(0);

        let gold_per_unit = colon(&mut gold) && gold.first() == Some(&b'u');

        let (effect, value) = match config.get(&format!("agrup.e{i}")).filter(|s| !s.is_empty()) {
            Some(text) => {
                let mut text = text.as_bytes();

                let effect = decimal(&mut text).unwrap_or(0).max(0);

                let value = if effect > 0 && colon(&mut text) {
                    decimal(&mut text).unwrap_or(-1)
                } else {
                    -1
                };

                (effect, value)
            }
            None => (config.integer("agrup.ef").max(0) as i32, -1),
        };

        return (!materials.is_empty() || amount > 0).then_some(Rule {
            materials,
            gold: amount,
            gold_per_unit,
            effect,
            value,
        });
    }

    None
}

#[derive(Debug, Clone, Copy)]

pub struct Pending {
    pub request: Request,
    pub source: Item,
    pub destination: Item,
}

impl Pending {
    pub fn confirm(self, entered: &Entered) -> Option<Request> {
        let Request::Move { src, dst } = self.request else {
            return None;
        };

        (super::item_move::item_at(entered, src) == Some(self.source)
            && super::item_move::item_at(entered, dst) == Some(self.destination))
        .then_some(self.request)
    }
}

pub fn intercept(panels: &mut Panels, entered: &Entered, request: Request) -> bool {
    let Request::Move {
        src: Coord {
            container: 1,
            slot: source,
        },
        dst: Coord {
            container: 1,
            slot: destination,
        },
    } = request
    else {
        return false;
    };

    if source == destination {
        return false;
    }

    let (Some(a), Some(b)) = (
        entered.carry.get(source as usize),
        entered.carry.get(destination as usize),
    ) else {
        return false;
    };

    if source >= 60 || destination >= 60 || a.index == 0 || a.index != b.index {
        return false;
    }

    let Some(rule) = rule(&panels.client_config, a.index) else {
        return false;
    };

    if rule.effect > 0
        && !a
            .effects
            .iter()
            .any(|e| e.id == rule.effect as u8 && (rule.value < 0 || e.value == rule.value as u8))
    {
        return false;
    }

    let cap = super::item_use::max_stack(&panels.items, i32::from(a.index));

    let (a_count, b_count) = (stack_size(a).max(1), stack_size(b).max(1));

    if cap <= 0 || a_count >= cap || b_count >= cap {
        return false;
    }

    let units = a_count.min(b_count).min(cap - a_count.max(b_count));

    if units <= 0 {
        return false;
    }

    if panels
        .boxes
        .open()
        .any(|(_, b)| b.kind == 0x27 && b.context == 0x5e)
    {
        return true;
    }

    let mut costs = String::new();

    for cost in &rule.materials {
        let count = cost
            .amount
            .wrapping_mul(if cost.per_unit { units } else { 1 });

        let name = panels
            .items
            .name(cost.item as usize)
            .unwrap_or_default()
            .replace('_', " ");

        let name = if name.is_empty() {
            format!("item {}", cost.item)
        } else {
            bounded(&name, 63)
        };

        append_cost(&mut costs, &bounded(&format!("{count} {name}"), 95));
    }

    if rule.gold > 0 {
        let amount = rule
            .gold
            .wrapping_mul(if rule.gold_per_unit { units } else { 1 });

        let gold = formatted(
            &bounded(&panels.strings.translate("ss_1848"), 127),
            &[(b'd', amount.to_string())],
        )
        .unwrap_or_else(|| format!("{amount} gold"));

        append_cost(&mut costs, &bounded(&gold, 159));
    }

    let text = confirmation_text(
        &panels.strings.translate("ss_1847"),
        &panels.strings.translate("ss_152"),
        units,
        &costs,
    );

    let prompt = msgbox::Request {
        caption: None,
        kind: 0x27,
        context: 0x5e,
        body: msgbox::Body::Text(text),
        flags: msgbox::FLAGS,
    };

    if let Some(slot) = panels.raise_box(&prompt, None) {
        panels.box_groupings[slot] = Some(Pending {
            request,
            source: *a,
            destination: *b,
        });
    }

    true
}

fn bounded(text: &str, max: usize) -> String {
    use formats::text::{Codepage, codepage};

    let bytes = codepage::encode(text, Codepage::Windows1252);

    let end = bytes
        .iter()
        .position(|&b| b == 0)
        .unwrap_or(bytes.len())
        .min(max);

    codepage::decode(&bytes[..end], Codepage::Windows1252)
}

fn formatted(template: &str, args: &[(u8, String)]) -> Option<String> {
    if template.is_empty() || template.starts_with('þ') {
        return None;
    }

    let mut chars = template.chars();

    let mut out = String::new();

    let mut at = 0;

    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);

            continue;
        }

        let spec = chars.next()?;

        if spec == '%' {
            out.push('%');

            continue;
        }

        let (expected, value) = args.get(at)?;

        if spec != char::from(*expected) {
            return None;
        }

        out.push_str(value);

        at += 1;
    }

    (at == args.len()).then_some(out)
}

fn append_cost(costs: &mut String, next: &str) {
    if !costs.is_empty() {
        costs.push_str(" + ");
    }

    costs.push_str(next);

    *costs = bounded(costs, 255);
}

fn confirmation_text(template: &str, question: &str, units: i32, costs: &str) -> String {
    let args = [(b'd', units.to_string()), (b's', costs.to_owned())];

    let mut text = bounded(
        &formatted(&bounded(template, 127), &args).unwrap_or_else(|| costs.to_owned()),
        255,
    );

    if !question.is_empty() && !question.starts_with('þ') {
        text.push(' ');

        text.push_str(&bounded(question, 127));

        text = bounded(&text, 255);
    }

    use formats::text::{Codepage, codepage};

    let mut bytes = codepage::encode(&text, Codepage::Windows1252);

    if bytes.len() >= 255 {
        let mut end = 252;

        if let Some(space) = (181..=252).rev().find(|&i| bytes[i] == b' ') {
            end = space;

            while end > 0 && matches!(bytes[end - 1], b' ' | b'+') {
                end -= 1;
            }
        }

        bytes.truncate(end);

        bytes.extend_from_slice(b"...");
    }

    codepage::decode(&bytes, Codepage::Windows1252)
}
