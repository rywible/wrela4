use std::collections::BTreeMap;

use super::{OperationId, OperationKind, ValueId};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct EClassId(u32);

impl EClassId {
    pub const fn raw(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct EnodeKey {
    kind: OperationKind,
    operands: Vec<ValueId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Enode {
    key: EnodeKey,
    source: OperationId,
}

impl Enode {
    pub fn new(kind: OperationKind, operands: Vec<ValueId>, source: OperationId) -> Self {
        Self {
            key: EnodeKey { kind, operands },
            source,
        }
    }

    fn key(&self) -> &EnodeKey {
        &self.key
    }

    pub fn source(&self) -> OperationId {
        self.source
    }
}

#[derive(Clone, Debug, Default)]
pub struct AeGraph {
    keys: BTreeMap<EnodeKey, EClassId>,
    enodes: Vec<Enode>,
}

impl AeGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn intern(&mut self, enode: Enode) -> EClassId {
        if let Some(id) = self.keys.get(enode.key()).copied() {
            return id;
        }
        let id = EClassId(self.enodes.len() as u32);
        self.keys.insert(enode.key().clone(), id);
        self.enodes.push(enode);
        id
    }

    pub fn enode_count(&self) -> usize {
        self.enodes.len()
    }

    pub fn enodes(&self) -> &[Enode] {
        &self.enodes
    }
}

#[cfg(test)]
mod tests {
    use super::{AeGraph, Enode};
    use crate::mir::{OperationId, OperationKind, ValueId};

    #[test]
    fn aegraph_interns_identical_pure_enodes() {
        let mut graph = AeGraph::new();
        let a = graph.intern(Enode::new(
            OperationKind::Binary("+".to_string()),
            vec![ValueId::new(0), ValueId::new(1)],
            OperationId::new(0),
        ));
        let b = graph.intern(Enode::new(
            OperationKind::Binary("+".to_string()),
            vec![ValueId::new(0), ValueId::new(1)],
            OperationId::new(1),
        ));

        assert_eq!(a, b);
        assert_eq!(graph.enode_count(), 1);
    }
}
