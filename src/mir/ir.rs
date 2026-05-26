use crate::source::Span;

use super::{BlockId, EffectSet, MirType, OperationFacts, OperationId, PlaceId, RegionId, ValueId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RegionKind {
    Lambda { symbol: String },
    Gamma { label: String },
    Theta(ThetaKind),
    Delta { label: String },
    Omega { symbol: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ThetaKind {
    Repeat,
    For,
    Drain,
    Reduce,
    Scan,
    Loop,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegionData {
    kind: RegionKind,
    blocks: Vec<BlockId>,
    source: Option<Span>,
}

impl RegionData {
    pub fn lambda(symbol: String) -> Self {
        Self {
            kind: RegionKind::Lambda { symbol },
            blocks: Vec::new(),
            source: None,
        }
    }

    pub fn omega(symbol: String) -> Self {
        Self {
            kind: RegionKind::Omega { symbol },
            blocks: Vec::new(),
            source: None,
        }
    }

    pub fn gamma(label: String) -> Self {
        Self {
            kind: RegionKind::Gamma { label },
            blocks: Vec::new(),
            source: None,
        }
    }

    pub fn delta(label: String) -> Self {
        Self {
            kind: RegionKind::Delta { label },
            blocks: Vec::new(),
            source: None,
        }
    }

    pub fn theta(kind: ThetaKind) -> Self {
        Self {
            kind: RegionKind::Theta(kind),
            blocks: Vec::new(),
            source: None,
        }
    }

    pub fn blocks(&self) -> &[BlockId] {
        &self.blocks
    }

    pub fn kind(&self) -> &RegionKind {
        &self.kind
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BlockData {
    name: String,
    arguments: Vec<ValueId>,
    operations: Vec<OperationId>,
}

impl BlockData {
    pub fn new(name: String) -> Self {
        Self {
            name,
            arguments: Vec::new(),
            operations: Vec::new(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn arguments(&self) -> &[ValueId] {
        &self.arguments
    }

    pub fn operations(&self) -> &[OperationId] {
        &self.operations
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValueData {
    ty: MirType,
    source: Option<Span>,
}

impl ValueData {
    pub fn new(ty: MirType) -> Self {
        Self { ty, source: None }
    }

    pub fn ty(&self) -> &MirType {
        &self.ty
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlaceData {
    name: String,
    ty: MirType,
}

impl PlaceData {
    pub fn new(name: String, ty: MirType) -> Self {
        Self { name, ty }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn ty(&self) -> &MirType {
        &self.ty
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum OperationKind {
    Literal(String),
    ReadValue(String),
    Binary(String),
    Let(String),
    Return,
    TableRows {
        table: String,
        rows: u64,
    },
    MaskRead {
        name: String,
        table: String,
        rows: u64,
    },
    MaskAllTrue {
        table: String,
        rows: u64,
    },
    MaskAllFalse {
        table: String,
        rows: u64,
    },
    MaskNot,
    MaskAnd,
    MaskOr,
    RowToken {
        table: String,
        rows: u64,
    },
    ReduceRows {
        table: String,
        mask: String,
        field: String,
        rows: u64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationData {
    kind: OperationKind,
    operands: Vec<ValueId>,
    results: Vec<ValueId>,
    effects: EffectSet,
    facts: OperationFacts,
}

impl OperationData {
    pub fn new(
        kind: OperationKind,
        operands: Vec<ValueId>,
        results: Vec<ValueId>,
        effects: EffectSet,
    ) -> Self {
        Self {
            kind,
            operands,
            results,
            effects,
            facts: OperationFacts::empty(),
        }
    }

    pub fn with_facts(mut self, facts: OperationFacts) -> Self {
        self.facts = facts;
        self
    }

    pub fn kind(&self) -> &OperationKind {
        &self.kind
    }

    pub fn operands(&self) -> &[ValueId] {
        &self.operands
    }

    pub fn results(&self) -> &[ValueId] {
        &self.results
    }

    pub fn effects(&self) -> EffectSet {
        self.effects
    }

    pub fn facts(&self) -> &OperationFacts {
        &self.facts
    }

    pub fn facts_mut(&mut self) -> &mut OperationFacts {
        &mut self.facts
    }

    pub fn replace_operand_uses(&mut self, old: ValueId, new: ValueId) {
        for operand in &mut self.operands {
            if *operand == old {
                *operand = new;
            }
        }
    }

    pub fn replace_kind(&mut self, kind: OperationKind) {
        self.kind = kind;
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MirModule {
    regions: Vec<RegionData>,
    blocks: Vec<BlockData>,
    operations: Vec<OperationData>,
    values: Vec<ValueData>,
    places: Vec<PlaceData>,
}

impl MirModule {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_region(&mut self, data: RegionData) -> RegionId {
        let id = RegionId::new(self.regions.len() as u32);
        self.regions.push(data);
        id
    }

    pub fn push_block(&mut self, region: RegionId, data: BlockData) -> BlockId {
        let id = BlockId::new(self.blocks.len() as u32);
        self.blocks.push(data);
        self.regions[region.raw() as usize].blocks.push(id);
        id
    }

    pub fn push_block_argument(&mut self, block: BlockId, value: ValueId) {
        self.blocks[block.raw() as usize].arguments.push(value);
    }

    pub fn push_operation(&mut self, block: BlockId, data: OperationData) -> OperationId {
        let id = OperationId::new(self.operations.len() as u32);
        self.operations.push(data);
        self.blocks[block.raw() as usize].operations.push(id);
        id
    }

    pub fn push_value(&mut self, data: ValueData) -> ValueId {
        let id = ValueId::new(self.values.len() as u32);
        self.values.push(data);
        id
    }

    pub fn push_place(&mut self, data: PlaceData) -> PlaceId {
        let id = PlaceId::new(self.places.len() as u32);
        self.places.push(data);
        id
    }

    pub fn region(&self, id: RegionId) -> &RegionData {
        &self.regions[id.raw() as usize]
    }

    pub fn block(&self, id: BlockId) -> &BlockData {
        &self.blocks[id.raw() as usize]
    }

    pub fn operation(&self, id: OperationId) -> &OperationData {
        &self.operations[id.raw() as usize]
    }

    pub fn operation_mut(&mut self, id: OperationId) -> &mut OperationData {
        &mut self.operations[id.raw() as usize]
    }

    pub fn mark_reduce_pair_fused(&mut self, first: OperationId, second: OperationId) {
        self.operation_mut(first)
            .facts_mut()
            .mark_fused_with(second);
        self.operation_mut(second)
            .facts_mut()
            .mark_fused_into(first);
    }

    pub fn value(&self, id: ValueId) -> &ValueData {
        &self.values[id.raw() as usize]
    }

    pub fn regions(&self) -> &[RegionData] {
        &self.regions
    }

    pub fn blocks(&self) -> &[BlockData] {
        &self.blocks
    }

    pub fn operations(&self) -> &[OperationData] {
        &self.operations
    }

    pub fn values(&self) -> &[ValueData] {
        &self.values
    }

    pub fn places(&self) -> &[PlaceData] {
        &self.places
    }

    pub fn replace_value_uses(&mut self, old: ValueId, new: ValueId) {
        for operation in &mut self.operations {
            operation.replace_operand_uses(old, new);
        }
    }

    pub fn set_block_operations(&mut self, block: BlockId, operations: Vec<OperationId>) {
        self.blocks[block.raw() as usize].operations = operations;
    }
}

#[cfg(test)]
impl MirModule {
    pub fn empty_for_test() -> Self {
        let mut module = Self::new();
        let region = module.push_region(RegionData::lambda("test".to_string()));
        module.push_block(region, BlockData::new("entry".to_string()));
        module
    }

    pub fn push_test_operation(&mut self, data: OperationData) -> OperationId {
        self.push_operation(BlockId::new(0), data)
    }

    pub fn replace_operation_kind_for_test(&mut self, index: usize, kind: OperationKind) {
        self.operations[index].replace_kind(kind);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mir::{EffectSet, MirType, ScalarType};

    #[test]
    fn module_assigns_dense_ids() {
        let mut module = MirModule::new();
        let region = module.push_region(RegionData::lambda("root.Foo.run".to_string()));
        let block = module.push_block(region, BlockData::new("entry".to_string()));
        let value = module.push_value(ValueData::new(MirType::Scalar(ScalarType::None)));
        let op = module.push_operation(
            block,
            OperationData::new(
                OperationKind::Return,
                Vec::new(),
                vec![value],
                EffectSet::empty(),
            ),
        );

        assert_eq!(region.raw(), 0);
        assert_eq!(block.raw(), 0);
        assert_eq!(value.raw(), 0);
        assert_eq!(op.raw(), 0);
        assert_eq!(module.region(region).blocks(), &[block]);
        assert_eq!(module.block(block).operations(), &[op]);
    }
}
