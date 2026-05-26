macro_rules! id_type {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
        pub struct $name(u32);

        impl $name {
            pub const fn new(raw: u32) -> Self {
                Self(raw)
            }

            pub const fn raw(self) -> u32 {
                self.0
            }
        }
    };
}

id_type!(RegionId);
id_type!(BlockId);
id_type!(OperationId);
id_type!(ValueId);
id_type!(PlaceId);
id_type!(TypeId);

#[cfg(test)]
mod tests {
    use super::{BlockId, OperationId, RegionId, ValueId};

    #[test]
    fn ids_expose_stable_raw_values() {
        assert_eq!(RegionId::new(7).raw(), 7);
        assert_eq!(BlockId::new(8).raw(), 8);
        assert_eq!(OperationId::new(9).raw(), 9);
        assert_eq!(ValueId::new(10).raw(), 10);
    }
}
