use super::{
    Failure, invalid,
    quotes::{BlockRegions, Occurrence, Quotes, Region},
    tokens::Line,
};
use mtg_oracle_ast::printed::{ImplicitReason, PrintedNode, TokenRun};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    ops::Range,
};

type Extent = BTreeSet<Occurrence>;

#[derive(Default)]
pub(super) struct BlockNodes<'a> {
    pub ast: Option<&'a Value>,
    nodes: BTreeMap<String, Node<'a>>,
    pub(super) quotes: BlockRegions,
}

struct Node<'a> {
    value: &'a Value,
    parent: Option<String>,
    depth: usize,
    scope: String,
    extent: Extent,
}

pub(super) fn pointer(path: &str, key: &str) -> String {
    format!("{path}/{}", key.replace('~', "~0").replace('/', "~1"))
}

fn ability(value: &Value) -> bool {
    matches!(
        value["kind"].as_str(),
        Some(
            "chapter"
                | "max-speed"
                | "infinity"
                | "visit"
                | "sticker"
                | "exert-on-attack"
                | "linked-triggers"
                | "class-level-bar"
                | "case-solve"
                | "case-solved"
                | "anchor-words"
                | "keywords"
                | "activated"
                | "loyalty"
                | "triggered"
                | "static"
                | "spell"
                | "additional-cost"
        )
    )
}

fn visit<'a>(
    value: &'a Value,
    path: &str,
    parent: Option<&str>,
    scope: &str,
    depth: usize,
    nodes: &mut BTreeMap<String, Node<'a>>,
) {
    match value {
        Value::Array(array) => {
            for (index, item) in array.iter().enumerate() {
                visit(
                    item,
                    &pointer(path, &index.to_string()),
                    parent,
                    scope,
                    depth,
                    nodes,
                );
            }
        }
        Value::Object(object) => {
            let scope = if ability(value) { path } else { scope };
            nodes.insert(
                path.to_owned(),
                Node {
                    value,
                    parent: parent.map(str::to_owned),
                    depth,
                    scope: scope.to_owned(),
                    extent: Extent::new(),
                },
            );
            let opens_scope = value["effect"] == "create-reflexive-trigger"
                || value["effect"] == "create-delayed-trigger"
                    && value["targets"] == "delayed-ability";
            for (key, child) in object {
                let scope = if key == "effects" && opens_scope {
                    path
                } else {
                    scope
                };
                visit(
                    child,
                    &pointer(path, key),
                    Some(path),
                    scope,
                    depth + 1,
                    nodes,
                );
            }
        }
        _ => {}
    }
}

/// Token runs must be in the block and in bounds, nonempty, strictly
/// ordered, non-overlapping and non-mergeable (adjacent runs on one line).
fn extent(
    runs: &[TokenRun],
    block: Range<usize>,
    lines: &[Line<'_>],
) -> Result<Extent, &'static str> {
    let mut extent = Extent::new();
    let mut previous: Option<[usize; 3]> = None;
    for &TokenRun(run) in runs {
        let [line, start, end] = run.map(|v| v as usize);
        if !block.contains(&line) || !lines.get(line).is_some_and(|line| end <= line.tokens.len()) {
            return Err("token run out of bounds");
        }
        if start >= end {
            return Err("empty token run");
        }
        if let Some(prev) = previous {
            match (prev[0], prev[1], prev[2]).cmp(&(line, start, end)) {
                std::cmp::Ordering::Equal => return Err("duplicate token run"),
                std::cmp::Ordering::Greater => return Err("token runs out of order"),
                std::cmp::Ordering::Less if prev[0] == line && prev[2] > start => {
                    return Err("overlapping token runs");
                }
                std::cmp::Ordering::Less if prev[0] == line && prev[2] == start => {
                    return Err("mergeable adjacent token runs");
                }
                std::cmp::Ordering::Less => {}
            }
        }
        extent.extend((start..end).map(|index| (line, index)));
        previous = Some([line, start, end]);
    }
    Ok(extent)
}

fn implicit(value: &Value, reason: &ImplicitReason) -> bool {
    match reason {
        ImplicitReason::DefaultZone => {
            matches!(value["zone"].as_str(), Some("battlefield" | "library"))
        }
        ImplicitReason::ImperativeController => value["player"] == "you",
        ImplicitReason::ImpliedSingularTarget => value["amount"] == "fixed" && value["value"] == 1,
    }
}

pub(super) fn path(node: &PrintedNode) -> &str {
    match node {
        PrintedNode::Alias(v) => &v.path,
        PrintedNode::Hull(v) => &v.path,
        PrintedNode::Implicit(v) => &v.path,
        PrintedNode::Recorded(v) => &v.path,
        PrintedNode::Unlocated(v) => &v.path,
    }
}

fn verify_quotes(
    nodes: &BTreeMap<String, Node<'_>>,
    printed: &BTreeMap<&str, &PrintedNode>,
    quotes: &Quotes,
) -> Result<BlockRegions, Failure> {
    let mut regions = BlockRegions::default();
    let mut grants = BTreeMap::new();
    let fail = |path: &str, reason: &str| invalid("P6", format!("node {path:?}: {reason}"));

    // Parents precede descendants in JSON pointer order. Only the body of an
    // AST quoted grant enters a deeper region. Bind it to matching rebuilt
    // quote marks in the grant wrapper, independently of the body's runs.
    for (path, node) in nodes {
        let region = node
            .parent
            .as_deref()
            .map_or(Some(Region::Outside), |parent| {
                if nodes[parent].value["gained"] == "quoted" && *path == pointer(parent, "ability")
                {
                    regions.get(path)
                } else {
                    regions.get(parent)
                }
            });
        if region != Some(Region::Outside) {
            regions.enter(path.clone(), region);
        }
        if node.value["gained"] != "quoted" {
            continue;
        }
        let child = if node.extent.is_empty() {
            None
        } else {
            let mut pairs = node.extent.iter().filter_map(|&open| {
                let (parent, close) = quotes.enclosure(open)?;
                (Some(parent) == regions.get(path) && node.extent.contains(&close)).then_some(open)
            });
            let open = pairs
                .next()
                .filter(|_| pairs.next().is_none())
                .ok_or_else(|| fail(path, "quoted grant has no unique enclosing quote pair"))?;
            if grants.insert(open, path.as_str()).is_some() {
                return Err(fail(path, "quoted grant reuses an enclosure"));
            }
            Some(Region::Inside(open))
        };
        regions.enter(pointer(path, "ability"), child);
    }

    for (path, entry) in printed {
        let region = regions.get(path);
        let check = |runs: &[TokenRun], field: &str| {
            let contains = |(line, index)| {
                runs.iter().any(|run| {
                    let [l, start, end] = run.0.map(|n| n as usize);
                    l == line && (start..end).contains(&index)
                })
            };
            for &TokenRun([line, start, end]) in runs {
                for index in start..end {
                    let occurrence = (line as usize, index as usize);
                    let mut actual = quotes.region(occurrence);
                    while Some(actual) != region {
                        let proven_parent = |open| {
                            let (parent, close) = quotes.enclosure(open)?;
                            let owner = *grants.get(&open)?;
                            (contains(open)
                                && contains(close)
                                && (path.is_empty()
                                    || owner == *path
                                    || owner
                                        .strip_prefix(path)
                                        .is_some_and(|tail| tail.starts_with('/'))))
                            .then_some(parent)
                        };
                        let Region::Inside(open) = actual else {
                            return Err(fail(
                                path,
                                &format!(
                                    "{field} escapes its quote region at token {occurrence:?}"
                                ),
                            ));
                        };
                        actual = proven_parent(open).ok_or_else(|| {
                            fail(path, &format!("{field} crosses an unowned or split quote at token {occurrence:?}"))
                        })?;
                    }
                }
            }
            Ok(())
        };
        match entry {
            PrintedNode::Recorded(node) => {
                check(&node.span, "span")?;
                check(node.folds.as_deref().unwrap_or_default(), "folds")?;
            }
            PrintedNode::Hull(node) => {
                check(&node.span, "hull span")?;
                check(node.clauses.as_deref().unwrap_or_default(), "clauses")?;
            }
            PrintedNode::Alias(node) if regions.get(&node.of) != region => {
                return Err(fail(path, "alias crosses a quote region"));
            }
            _ => {}
        }
    }
    Ok(regions)
}

impl<'a> BlockNodes<'a> {
    pub(super) fn verify(
        ast: &'a Value,
        entries: &[PrintedNode],
        block: Range<usize>,
        lines: &[Line<'_>],
        quotes: &Quotes,
    ) -> Result<Self, Failure> {
        let mut nodes = BTreeMap::new();
        if !ast.is_object() {
            return Err(invalid("P6", "legacy block has no AST object"));
        }
        visit(ast, "", None, "", 0, &mut nodes);
        let mut printed = BTreeMap::new();
        for entry in entries {
            if !nodes.contains_key(path(entry)) || printed.insert(path(entry), entry).is_some() {
                return Err(invalid(
                    "P6",
                    format!("unexpected or duplicate node {:?}", path(entry)),
                ));
            }
        }
        if printed.len() != nodes.len() {
            return Err(invalid("P6", "printed node inventory differs from the AST"));
        }
        let mut children = BTreeMap::<String, Extent>::new();
        let mut ordered: Vec<_> = nodes.keys().cloned().collect();
        ordered.sort_by_key(|path| std::cmp::Reverse(nodes[path].depth));
        for path in ordered {
            let fail = |reason: &str| invalid("P6", format!("node {path:?}: {reason}"));
            let get_extent = |runs: &[TokenRun], field: &str| {
                extent(runs, block.clone(), lines).map_err(|why| fail(&format!("{field}: {why}")))
            };
            let child = children.remove(&path).unwrap_or_default();
            let node = &nodes[&path];
            let value = match printed[path.as_str()] {
                PrintedNode::Recorded(entry) => {
                    let mut span = get_extent(&entry.span, "span")?;
                    let folds = get_extent(entry.folds.as_deref().unwrap_or_default(), "folds")?;
                    if !span.is_disjoint(&folds) {
                        return Err(fail("fold overlaps recorded tokens"));
                    }
                    span.extend(folds);
                    if span.is_empty() || !child.is_subset(&span) {
                        return Err(fail("recorded tokens do not contain child extents"));
                    }
                    span
                }
                PrintedNode::Hull(entry) => {
                    let span = get_extent(&entry.span, "span")?;
                    let clauses =
                        get_extent(entry.clauses.as_deref().unwrap_or_default(), "clauses")?;
                    if !clauses.is_subset(&span) {
                        return Err(fail("hull clause run outside the hull span"));
                    }
                    if !clauses.is_disjoint(&child) {
                        return Err(fail("hull clause run overlaps a child extent"));
                    }
                    let mut expected = child;
                    expected.extend(clauses);
                    if span.is_empty() || span != expected {
                        return Err(fail(
                            "hull differs from child extents and attributed clauses",
                        ));
                    }
                    span
                }
                PrintedNode::Alias(entry) => {
                    if path.rsplit_once('/').is_some_and(|(array, index)| {
                        index.parse::<usize>().is_ok()
                            && entry.of.rsplit_once('/').is_some_and(|(parent, other)| {
                                parent == array && other.parse::<usize>().is_ok()
                            })
                    }) {
                        return Err(fail("alias duplicates an occurrence in the same list"));
                    }
                    let target = nodes.get(&entry.of);
                    if !target.is_some_and(|target| target.value == node.value)
                        || !matches!(
                            printed.get(entry.of.as_str()),
                            Some(
                                PrintedNode::Recorded(_)
                                    | PrintedNode::Hull(_)
                                    | PrintedNode::Implicit(_)
                            )
                        )
                        || !child.is_empty()
                    {
                        return Err(fail(
                            "alias does not resolve to an equal non-alias AST node",
                        ));
                    }
                    Extent::new()
                }
                PrintedNode::Implicit(entry)
                    if implicit(node.value, &entry.reason) && child.is_empty() =>
                {
                    Extent::new()
                }
                PrintedNode::Unlocated(_) if child.is_empty() => Extent::new(),
                _ => {
                    return Err(fail(
                        "unspanned node has an invalid default or spanned child",
                    ));
                }
            };
            if let Some(parent) = &node.parent {
                let siblings = children.entry(parent.clone()).or_default();
                if !siblings.is_disjoint(&value) {
                    return Err(fail("overlapping sibling extents"));
                }
                siblings.extend(value.iter().copied());
            }
            nodes.get_mut(&path).unwrap().extent = value;
        }
        let quotes = verify_quotes(&nodes, &printed, quotes)?;
        Ok(Self {
            ast: Some(ast),
            nodes,
            quotes,
        })
    }

    /// Innermost node covering every token in `run`, or the root (`""`) when
    /// none does, mirroring `printed/facts.ts`. A root-owned fact on tokens no
    /// node covers is not located evidence; consumers must not count it as such.
    pub(super) fn owner(&self, run: [u32; 3]) -> &str {
        let [line, start, end] = run.map(|v| v as usize);
        self.nodes
            .iter()
            .filter(|(_, node)| (start..end).all(|index| node.extent.contains(&(line, index))))
            .max_by_key(|(_, node)| node.depth)
            .map_or("", |(path, _)| path)
    }

    pub(super) fn scope(&self, path: &str) -> Option<&str> {
        self.nodes.get(path).map(|node| node.scope.as_str())
    }

    pub(super) fn value(&self, path: &str) -> Option<&Value> {
        self.ast?.pointer(path)
    }
}
