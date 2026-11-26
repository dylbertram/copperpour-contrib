//! Saved embedded-symbol export metadata. This does not read external libraries.

use super::super::sch_hierarchy::natural_compare;
use super::super::symbol_pins::{stacked, Stacked};
use copperpour_sexp::schematic::{parse_at, parse_subsymbol_unit, symbol_instance, SymbolInstance};
use copperpour_sexp::SexpNode;
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

type Unit = u32;
type PinText<'a> = [Option<&'a str>; 2];
type PinNumber<'a> = Cow<'a, str>;
type LibraryPins<'a> = BTreeMap<PinNumber<'a>, Vec<PinText<'a>>>;
type PinRoster<'a> = Vec<(Unit, u32, &'a str, PinText<'a>)>;
type SavedPinOrder<'a> = Vec<(&'a str, (f64, f64))>;
type SavedDrawOrder<'a> = Vec<(Unit, u32, &'a SexpNode)>;
type Identities<'a> = BTreeMap<&'a str, &'a SexpNode>;
type Contexts<'a> = BTreeSet<(&'a str, &'a str)>;

struct Metadata<'a> {
    duplicate_pin_numbers_are_jumpers: bool,
    jumper_groups: Vec<BTreeSet<&'a str>>,
    unit_count: Unit,
    unit_names: BTreeMap<Unit, &'a str>,
    unit_pins: BTreeMap<Unit, UnitPins<'a>>,
    common_pins: UnitPins<'a>,
    fields: BTreeMap<&'a str, Vec<&'a str>>,
    library_pins: LibraryPins<'a>,
    library_pin_order: Vec<PinNumber<'a>>,
    duplicate_pin_roster: Option<PinRoster<'a>>,
    tied_draw_order: Option<SavedDrawOrder<'a>>,
}

fn item_property(node: &SexpNode) -> Option<(&str, &str)> {
    let start = if matches!(node.get(1), Some(SexpNode::Atom(s)) if s == "private") {
        2
    } else {
        1
    };
    Some((node.get(start)?.as_str()?, node.get(start + 1)?.as_str()?))
}

fn exported_fields(node: &SexpNode) -> BTreeMap<&str, Vec<&str>> {
    let mut fields = BTreeMap::<_, Vec<_>>::new();
    for (name, value) in node
        .find_all("property")
        .into_iter()
        .filter_map(item_property)
    {
        match name {
            // Embedded description, keywords and footprint filters are tracked in #938.
            "Description" | "ki_description" | "ki_keywords" | "ki_fp_filters" | "ki_locked" => {}
            _ => fields.entry(name).or_default().push(value),
        }
    }
    for name in ["Reference", "Value", "Footprint", "Datasheet"] {
        fields.entry(name).or_insert_with(|| vec![""]);
    }
    fields
}

fn default_unit_name(mut unit: Unit) -> String {
    let mut letters = Vec::new();
    while unit > 0 {
        let digit = (unit - 1) % 26;
        letters.push(char::from(b'A' + digit as u8));
        unit = (unit - digit) / 26;
    }
    letters.into_iter().rev().collect()
}

struct Pin<'a> {
    number: &'a str,
    text: PinText<'a>,
    at: (f64, f64),
}

impl<'a> Pin<'a> {
    fn from_node(node: &'a SexpNode) -> Option<Self> {
        let (x, y, _) = parse_at(node)?;
        Some(Self {
            number: node.find_str("number")?,
            text: [
                node.find_str("name"),
                node.get(1).and_then(SexpNode::as_str),
            ],
            at: (x, y),
        })
    }
}

fn expanded_numbers(number: &str) -> Vec<PinNumber<'_>> {
    if !number.starts_with('[') {
        return vec![Cow::Borrowed(number)];
    }
    let parts = stacked(number);
    // Keep a conservative literal for unusually large ranges. Do not let a
    // short saved record allocate a list of unbounded length.
    const MAX_EXPANDED: u64 = 4096;
    let count = parts.iter().try_fold(0u64, |count, part| {
        let len = match part {
            Stacked::One(_) => 1,
            Stacked::Run(_, first, last) => first.abs_diff(*last).saturating_add(1),
        };
        count.checked_add(len)
    });
    if count.is_none_or(|n| n > MAX_EXPANDED) {
        return vec![Cow::Borrowed(number)];
    }
    let mut numbers = Vec::new();
    for part in parts {
        match part {
            Stacked::One(value) => numbers.push(Cow::Owned(value)),
            Stacked::Run(prefix, first, last) => {
                numbers.extend((first..=last).map(|n| Cow::Owned(format!("{prefix}{n}"))));
            }
        }
    }
    numbers
}

#[derive(PartialEq)]
struct UnitPins<'a> {
    numbers: Vec<PinNumber<'a>>,
    coincident_input: Option<SavedPinOrder<'a>>,
}

fn unit_pins<'p, 'a: 'p>(pins: impl Iterator<Item = &'p Pin<'a>>) -> UnitPins<'a> {
    let saved: Vec<_> = pins.collect();
    let mut pins = saved.clone();
    // The exported unit list is ordered by library pin position, left to right,
    // then top to bottom. The absolute position itself is not metadata.
    pins.sort_by(|a, b| a.at.0.total_cmp(&b.at.0).then(b.at.1.total_cmp(&a.at.1)));
    // KiCad's position sort is unstable. With tied positions, changes anywhere
    // in the sort input can change the order in which tied numbers are emitted.
    let coincident_input = pins
        .windows(2)
        .any(|pair| pair[0].at == pair[1].at && pair[0].number != pair[1].number)
        .then(|| saved.into_iter().map(|pin| (pin.number, pin.at)).collect());
    let mut seen = BTreeSet::new();
    let numbers = pins
        .into_iter()
        .flat_map(|pin| expanded_numbers(pin.number))
        .filter(|number| seen.insert(number.clone()))
        .collect();
    UnitPins {
        numbers,
        coincident_input,
    }
}

impl<'a> Metadata<'a> {
    fn from_node(node: &'a SexpNode) -> Self {
        let jumper_groups = node
            .find_all("jumper_pin_groups")
            .into_iter()
            .flat_map(|group| group.children().unwrap_or_default().iter().skip(1))
            .filter_map(SexpNode::children)
            .map(|group| group.iter().filter_map(SexpNode::as_str).collect())
            .collect();
        let mut unit_count = 1;
        let mut unit_names = BTreeMap::new();
        let mut pins_by_unit = BTreeMap::<Unit, Vec<Pin<'_>>>::new();
        let mut library_pins_by_number = BTreeMap::<_, Vec<_>>::new();
        let mut drawn_pins = Vec::new();
        let mut saved_pins = Vec::new();
        for block in node.find_all("symbol") {
            let Some(name) = block.get(1).and_then(SexpNode::as_str) else {
                continue;
            };
            let Some(unit) = parse_subsymbol_unit(name) else {
                continue;
            };
            let style: u32 = name.rsplit('_').next().unwrap().parse().unwrap_or_default();
            unit_count = unit_count.max(unit);
            if let Some(display_name) = block.find_str("unit_name") {
                unit_names.insert(unit, display_name);
            }
            for pin_node in block.find_all("pin") {
                let Some(pin) = Pin::from_node(pin_node) else {
                    continue;
                };
                drawn_pins.push((unit, style, pin.number, pin.text));
                saved_pins.push((unit, style, pin_node));
                library_pins_by_number
                    .entry(pin.number)
                    .or_default()
                    .push((unit, style, pin.at, pin.text));
                pins_by_unit.entry(unit).or_default().push(pin);
            }
        }
        // KiCad's number sort is natural and case-insensitive, but duplicate
        // removal also requires adjacent entries with exactly the same text.
        // Preserve the roster when unequal numbers can tie in that sort.
        let mut ordered_pins: Vec<_> = library_pins_by_number
            .into_iter()
            .map(|(number, mut entries)| {
                // Compare the candidates in drawing order. Different
                // candidates for one number can change which pin is exported.
                entries.sort_by(|a, b| {
                    (a.0, a.1)
                        .cmp(&(b.0, b.1))
                        .then(a.2 .0.total_cmp(&b.2 .0))
                        .then(b.2 .1.total_cmp(&a.2 .1))
                });
                let mut text: Vec<_> = entries.into_iter().map(|(_, _, _, text)| text).collect();
                if text.windows(2).all(|pair| pair[0] == pair[1]) {
                    text.truncate(1);
                }
                (number.to_uppercase(), number, text)
            })
            .collect();
        let has_conflicting_pins = ordered_pins.iter().any(|(_, _, pins)| pins.len() > 1);
        ordered_pins.sort_by(|(a, _, _), (b, _, _)| natural_compare(a, b));
        let has_tied_numbers = ordered_pins
            .windows(2)
            .any(|pair| natural_compare(&pair[0].0, &pair[1].0).is_eq());
        let mut library_pins = LibraryPins::new();
        let mut library_pin_order = Vec::new();
        for (_, raw, entries) in ordered_pins {
            for number in expanded_numbers(raw) {
                library_pin_order.push(number.clone());
                library_pins.entry(number).or_default().extend(&entries);
            }
        }
        // Compare the saved draw order. A byte-order sort would lose the order
        // of distinct spellings that KiCad treats as equal, such as 8 and 08.
        let duplicate_pin_roster = (has_conflicting_pins || has_tied_numbers).then_some(drawn_pins);
        unit_names.retain(|&unit, name| unit != 0 && *name != default_unit_name(unit));
        let common = pins_by_unit.remove(&0).unwrap_or_default();
        let common_pins = unit_pins(common.iter());
        let unit_pins = pins_by_unit
            .iter()
            .map(|(&unit, pins)| (unit, unit_pins(common.iter().chain(pins))))
            .filter(|(_, pins)| *pins != common_pins)
            .collect();
        Self {
            duplicate_pin_numbers_are_jumpers: node.find_str("duplicate_pin_numbers_are_jumpers")
                == Some("yes"),
            jumper_groups,
            unit_count,
            unit_names,
            unit_pins,
            common_pins,
            fields: exported_fields(node),
            library_pins,
            library_pin_order,
            duplicate_pin_roster,
            tied_draw_order: has_tied_numbers.then_some(saved_pins),
        }
    }

    fn pins(&self, unit: &Unit) -> &UnitPins<'a> {
        self.unit_pins.get(unit).unwrap_or(&self.common_pins)
    }

    fn pin_list_changed(&self, other: &Self, unit: &Unit) -> bool {
        let (old, new) = (self.pins(unit), other.pins(unit));
        if old != new {
            return true;
        }
        // KiCad sorts the full draw list before filtering to this unit. With
        // ties in that sort and in the unit's position sort, pins in another
        // unit can affect this one, including when it comes from a local alias.
        old.coincident_input.is_some() && self.tied_draw_order != other.tied_draw_order
    }

    fn unit_changes(&self, other: &Self, unit: Unit, subject: &str, reasons: &mut Vec<String>) {
        if self.unit_names.get(&unit) != other.unit_names.get(&unit) {
            reasons.push(format!("{subject} changed embedded unit {unit} name"));
        }
        if self.pin_list_changed(other, &unit) {
            reasons.push(format!("{subject} changed embedded unit {unit} pin list"));
        }
    }

    fn append_changes(
        &self,
        other: &Self,
        subject: &str,
        overlaid_units: &BTreeSet<Unit>,
        reasons: &mut Vec<String>,
    ) {
        if self.duplicate_pin_numbers_are_jumpers != other.duplicate_pin_numbers_are_jumpers {
            reasons.push(format!(
                "{subject} changed duplicate_pin_numbers_are_jumpers"
            ));
        }
        if self.jumper_groups != other.jumper_groups {
            reasons.push(format!("{subject} changed jumper pin groups"));
        }
        if self.unit_count != other.unit_count {
            reasons.push(format!("{subject} changed embedded unit count"));
        }
        for unit in union_keys(&self.unit_names, &other.unit_names) {
            if !overlaid_units.contains(unit)
                && self.unit_names.get(unit) != other.unit_names.get(unit)
            {
                reasons.push(format!("{subject} changed embedded unit {unit} name"));
            }
        }
        let mut units = union_keys(&self.unit_pins, &other.unit_pins);
        units.extend(overlaid_units);
        if self.pin_list_changed(other, &0)
            && units.len() < self.unit_count.max(other.unit_count) as usize
        {
            reasons.push(format!("{subject} changed common unit pin list"));
        }
        for unit in units {
            if !overlaid_units.contains(unit) && self.pin_list_changed(other, unit) {
                reasons.push(format!("{subject} changed embedded unit {unit} pin list"));
            }
        }
        for name in union_keys(&self.fields, &other.fields) {
            if self.fields.get(name) != other.fields.get(name) {
                reasons.push(format!("{subject} changed library field {name:?}"));
            }
        }
        for number in union_keys(&self.library_pins, &other.library_pins) {
            if self.library_pins.get(number) != other.library_pins.get(number) {
                reasons.push(format!("{subject} changed library pin {number:?} metadata"));
            }
        }
        let same_library_pins = self.library_pins == other.library_pins;
        if same_library_pins && self.duplicate_pin_roster != other.duplicate_pin_roster {
            reasons.push(format!(
                "{subject} cannot establish unchanged library metadata for duplicate pin numbers"
            ));
        }
        if same_library_pins && self.library_pin_order != other.library_pin_order {
            reasons.push(format!("{subject} changed library pin record order"));
        }
    }
}

fn union_keys<'a, K: Ord, V>(a: &'a BTreeMap<K, V>, b: &'a BTreeMap<K, V>) -> BTreeSet<&'a K> {
    a.keys().chain(b.keys()).collect()
}

struct Library<'a> {
    symbols: BTreeMap<&'a str, &'a SexpNode>,
    metadata: BTreeMap<&'a str, Metadata<'a>>,
}

impl<'a> Library<'a> {
    fn from_sheet(tree: &'a SexpNode) -> Self {
        Self {
            // KiCad replaces an earlier entry with the same embedded key.
            symbols: tree
                .find("lib_symbols")
                .map(|node| node.find_all("symbol"))
                .unwrap_or_default()
                .into_iter()
                .filter_map(|node| Some((node.get(1)?.as_str()?, node)))
                .collect(),
            metadata: BTreeMap::new(),
        }
    }

    fn metadata(&mut self, node: &'a SexpNode) -> &Metadata<'a> {
        let name = node.get(1).and_then(SexpNode::as_str).unwrap_or_default();
        self.metadata
            .entry(name)
            .or_insert_with(|| Metadata::from_node(node))
    }
}

#[derive(PartialEq)]
enum ExportScope {
    Primary(BTreeSet<Unit>),
    Secondary(Unit),
}

/// Narrow comparisons only when the saved placements identify the same unique
/// component in every recorded path. Otherwise keep comparing whole definitions.
fn export_scopes<'a>(
    symbols: &Identities<'a>,
    library: &Library<'a>,
) -> BTreeMap<&'a str, ExportScope> {
    let mut groups = BTreeMap::<_, Vec<_>>::new();
    for (&uuid, node) in symbols {
        let Some(instance) = symbol_instance(node) else {
            continue;
        };
        let contexts = stable_contexts(node, &instance);
        groups
            .entry(instance.reference.clone())
            .or_default()
            .push((uuid, instance, contexts));
    }
    let mut scopes = BTreeMap::new();
    for group in groups.values() {
        let [(primary_uuid, primary, Some(primary_paths)), secondaries @ ..] = group.as_slice()
        else {
            continue;
        };
        let mut units = BTreeSet::from([primary.unit]);
        let count = |instance: &SymbolInstance| {
            library
                .symbols
                .get(instance.lib_symbol_name())
                .map_or(0, |node| {
                    node.find_all("symbol")
                        .into_iter()
                        .filter_map(|block| parse_subsymbol_unit(block.get(1)?.as_str()?))
                        .max()
                        .unwrap_or(1)
                })
        };
        let primary_count = count(primary);
        if secondaries.iter().all(|(_, instance, paths)| {
            paths.as_ref() == Some(primary_paths)
                && instance.lib_id == primary.lib_id
                && units.insert(instance.unit)
                && instance.unit > 0
                && instance.unit <= primary_count
                && instance.unit <= count(instance)
        }) {
            units.remove(&primary.unit);
            scopes.insert(*primary_uuid, ExportScope::Primary(units));
            scopes.extend(
                secondaries
                    .iter()
                    .map(|(uuid, instance, _)| (*uuid, ExportScope::Secondary(instance.unit))),
            );
        }
    }
    scopes
}

fn stable_contexts<'a>(node: &'a SexpNode, instance: &SymbolInstance) -> Option<Contexts<'a>> {
    let mut contexts = BTreeSet::new();
    for project in node.find("instances")?.find_all("project") {
        let name = project.get(1)?.as_str()?;
        for path in project.find_all("path") {
            if path.find_str("reference")? != instance.reference
                || path.find_str("unit")?.parse::<Unit>().ok()? != instance.unit
                || !contexts.insert((name, path.get(1)?.as_str()?))
            {
                return None;
            }
        }
    }
    (!contexts.is_empty()).then_some(contexts)
}

pub(super) fn append_changes(
    before: (&SexpNode, &Identities<'_>),
    after: (&SexpNode, &Identities<'_>),
    reasons: &mut Vec<String>,
) {
    let (mut old, mut new) = (Library::from_sheet(before.0), Library::from_sheet(after.0));
    let old_scopes = export_scopes(before.1, &old);
    let new_scopes = export_scopes(after.1, &new);
    let no_overlays = BTreeSet::new();
    for (&uuid, symbol) in before.1 {
        let Some(kept) = after.1.get(uuid) else {
            continue;
        };
        let (Some(was), Some(now)) = (symbol_instance(symbol), symbol_instance(kept)) else {
            continue;
        };
        if was.reference.starts_with('#') {
            continue;
        }
        let subject = format!("symbol {uuid} ({})", was.reference);
        let old_name = saved_override(was.lib_name.as_deref(), &was.lib_id);
        let new_name = saved_override(now.lib_name.as_deref(), &now.lib_id);
        let (was_lib, now_lib) = (
            old.symbols.get(old_name.unwrap_or(&was.lib_id)).copied(),
            new.symbols.get(new_name.unwrap_or(&now.lib_id)).copied(),
        );
        let scope = old_scopes
            .get(uuid)
            .filter(|&scope| Some(scope) == new_scopes.get(uuid));
        if let Some(ExportScope::Secondary(unit)) = scope {
            if let (Some(was_lib), Some(now_lib)) = (was_lib, now_lib) {
                old.metadata(was_lib)
                    .unit_changes(new.metadata(now_lib), *unit, &subject, reasons);
            }
            continue;
        }
        if old_name != new_name {
            reasons.push(format!(
                "{subject} changed embedded library identity (lib_name)"
            ));
        }
        // Missing definitions are already refused by the connectivity check.
        if let (Some(was_lib), Some(now_lib)) = (was_lib, now_lib) {
            let overlaid_units = match scope {
                Some(ExportScope::Primary(units)) => units,
                _ => &no_overlays,
            };
            old.metadata(was_lib).append_changes(
                new.metadata(now_lib),
                &subject,
                overlaid_units,
                reasons,
            );
        }
    }
}

fn saved_override<'a>(name: Option<&'a str>, lib_id: &str) -> Option<&'a str> {
    name.filter(|name| !name.is_empty() && *name != lib_id)
}
