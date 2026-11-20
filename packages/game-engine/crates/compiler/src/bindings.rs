use super::*;
use serde_json::{Value, json};

// Construct the expected diagnostic graph from executable operands, never the reverse.
pub(super) struct Bindings<'a> {
    identity: &'a str,
    nodes: Vec<Value>,
    references: Vec<Value>,
}

impl<'a> Bindings<'a> {
    pub(super) fn new(identity: &'a str, source: &Value) -> Self {
        let mut graph = Self {
            identity,
            nodes: vec![],
            references: vec![],
        };
        let object = graph.node(
            "/source",
            "object",
            "source",
            json!({"identity":identity,"rules":["201.5"]}),
        );
        graph.node("/faces/0", "face", "face", json!({"source":object,"sourceFaceIndex":0,"name":source["name"],
            "layout":source["layout"],"typeLine":source["type_line"],"manaCost":source["mana_cost"],"rules":["712"]}));
        graph
    }

    pub(super) fn id(&self, path: &str, suffix: &str) -> String {
        format!("card:{}{path}#{suffix}", self.identity)
    }

    pub(super) fn node(
        &mut self,
        path: &str,
        suffix: &str,
        kind: &str,
        mut fields: Value,
    ) -> String {
        let id = self.id(path, suffix);
        fields["id"] = json!(id);
        fields["path"] = json!(path);
        fields["node"] = json!(kind);
        self.nodes.push(fields);
        id
    }

    pub(super) fn reference(&mut self, path: &str, expression: &Value, target: &str, rule: &str) {
        self.reference_rules(path, expression, target, &[rule]);
    }

    pub(super) fn reference_rules(
        &mut self,
        path: &str,
        expression: &Value,
        target: &str,
        rules: &[&str],
    ) {
        self.references
            .push(json!({"path":path,"expression":expression,
            "binding":"bound","target":target,"rules":rules}));
    }

    pub(super) fn target(
        &mut self,
        scope: &str,
        path: &str,
        expression: &Value,
        selection: &str,
    ) -> String {
        let id = format!("{scope}/targets/{}", selection.replace(':', "%3A"));
        self.nodes.push(json!({"id":id,"path":path,"node":"selection","scope":scope,"expression":expression,
            "cardinality":"one","timing":"stack-announcement","selection":"target","rules":["601.2c"]}));
        self.reference(path, expression, &id, "601.2c");
        id
    }

    pub(super) fn ability(&mut self, path: &str, kind: &str, role: &str) {
        let ability = self.node(
            path,
            "ability",
            "ability",
            json!({
            "source":self.id("/source","object"),"face":self.id("/faces/0","face"),
            "abilityKind":kind,"you":self.id(path,"you"),"rules":["113","109.5"]}),
        );
        self.node(
            path,
            "you",
            "runtime-player",
            json!({"scope":ability,"role":role,"rules":["109.5"]}),
        );
    }

    pub(super) fn event(
        &mut self,
        base: &str,
        path: &str,
        operation: &str,
        input: &str,
        optional: bool,
    ) {
        let mut fields = json!({"scope":self.id(base,"ability"),"operation":operation,
            "inputs":[input],"availability":"when-performed","rules":["608.2c"]});
        if optional {
            fields["optional"] = json!(true);
            fields["decider"] = json!(self.id(base, "you"));
        }
        self.node(path, "event", "event", fields);
    }

    pub(super) fn result(
        &mut self,
        path: &str,
        kind: &str,
        identity: &str,
        description: Option<&Value>,
    ) {
        let mut fields = json!({"event":self.id(path,"event"),"result":kind,"cardinality":"one",
            "availability":"if-produced","identity":identity,"rules":["608.2h"]});
        if let Some(description) = description {
            fields["description"] = description.clone();
        }
        self.node(path, kind, "event-result", fields);
    }

    /// A card may have independently supported siblings. Compare every field of
    /// the shared source/face anchors and every node/reference owned by this block.
    pub(super) fn validate_block(&self, binding: &Value, root: &str) -> Result<(), Failure> {
        let fail = || unsupported("binding", "incomplete cast trigger binding graph");
        let prefix = format!("card:{}{root}", self.identity);
        let local_path = |path: &str| {
            path == root
                || path
                    .strip_prefix(root)
                    .is_some_and(|tail| tail.starts_with('/'))
        };
        let local_id = |id: &str| {
            id.strip_prefix(&prefix)
                .is_some_and(|tail| tail.starts_with(['#', '/']))
        };
        let mut scoped = binding.clone();
        let nodes = scoped["nodes"].as_array_mut().ok_or_else(fail)?;
        nodes.retain(|node| {
            node["path"]
                .as_str()
                .is_some_and(|path| matches!(path, "/source" | "/faces/0") || local_path(path))
                || node["id"]
                    .as_str()
                    .is_some_and(|id| local_id(id) || self.nodes.iter().any(|n| n["id"] == id))
                || node["scope"].as_str().is_some_and(local_id)
        });
        let refs = scoped["references"].as_array_mut().ok_or_else(fail)?;
        let total = refs.len();
        if binding["counts"] != json!({"obligations":total,"bound":total,"unresolved":0}) {
            return Err(fail());
        }
        refs.retain(|r| {
            r["path"].as_str().is_some_and(local_path) || r["target"].as_str().is_some_and(local_id)
        });
        let n = refs.len();
        scoped["counts"] = json!({"obligations":n,"bound":n,"unresolved":0});
        self.validate(&scoped, "incomplete cast trigger binding graph")
    }

    /// Archive v1 can report a scope gap even when every emitted reference is bound.
    /// Callers must independently bind the missing operand from the executable source.
    pub(super) fn with_scope_diagnostic(self, diagnostic: Value) -> Value {
        let count = self.references.len();
        json!({
            "artifactKind":"oracle-reference-binding","schemaVersion":1,"executable":false,
            "identity":self.identity, "rules":{"edition":RULES_EDITION,
                "url":"https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt"},
            "nodes":self.nodes,"references":self.references,"diagnostics":[diagnostic],
            "counts":{"obligations":count,"bound":count,"unresolved":0},
            "referencesBound":false,"fullyBound":false,"engineFeatures":[]
        })
    }

    #[cfg(test)]
    pub(super) fn synthetic_envelope(&self) -> Value {
        json!({"artifactKind":"oracle-reference-binding","schemaVersion":1,"executable":false,
            "rules":{"edition":RULES_EDITION,"url":"https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt"},
            "identity":self.identity,"fullyBound":true,"referencesBound":true,"diagnostics":[],"engineFeatures":[],
            "nodes":self.nodes,"references":self.references,"counts":{"obligations":self.references.len(),
            "bound":self.references.len(),"unresolved":0}})
    }

    pub(super) fn validate_ordered(&self, binding: &Value, message: &str) -> Result<(), Failure> {
        if binding["nodes"].as_array() != Some(&self.nodes)
            || binding["references"].as_array() != Some(&self.references)
        {
            return Err(unsupported("binding", message));
        }
        self.validate(binding, message)
    }

    pub(super) fn validate(&self, binding: &Value, message: &str) -> Result<(), Failure> {
        self.validate_with_features(binding, &[], message)
    }

    pub(super) fn validate_with_features(
        &self,
        binding: &Value,
        features: &[Value],
        message: &str,
    ) -> Result<(), Failure> {
        let same_members = |actual: &Value, expected: &[Value]| {
            actual.as_array().is_some_and(|items| {
                items.len() == expected.len() && expected.iter().all(|item| items.contains(item))
            })
        };
        // All twelve archive-v1 envelope fields are checked below.
        if binding.as_object().is_none_or(|fields| fields.len() != 12)
            || binding["artifactKind"] != "oracle-reference-binding"
            || binding["schemaVersion"] != 1
            || binding["executable"] != false
            || binding["rules"]
                != json!({"edition":RULES_EDITION,
                "url":"https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt"})
            || binding["identity"] != self.identity
            || binding["fullyBound"] != features.is_empty()
            || binding["referencesBound"] != true
            || binding["diagnostics"] != json!([])
            || !same_members(&binding["engineFeatures"], features)
            || binding["counts"]
                != json!({"obligations":self.references.len(),"bound":self.references.len(),"unresolved":0})
            || !same_members(&binding["nodes"], &self.nodes)
            || !same_members(&binding["references"], &self.references)
        {
            return Err(unsupported("binding", message));
        }
        Ok(())
    }
}
