
use serde::{Deserialize, Deserializer, Serializer};
use std::collections::HashMap;
use serde::Serialize;

pub mod utilization {

    use super::*;

    pub fn serialize<S>(array: &[usize], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = HashMap::new();
        for (index, &value) in array.iter().enumerate() {
            if value != 0 && value != 1 {
                map.insert(index, value);
            }
        }
        map.serialize(serializer)
    }

    #[allow(dead_code)] // Maybe needed in the future
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<usize>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let map: HashMap<usize, usize> = HashMap::deserialize(deserializer)?;
        let max_index = map.keys().max().unwrap_or(&0);
        let mut array = vec![0; max_index + 1];
        for (&index, &value) in map.iter() {
            array[index] = value;
        }
        Ok(array)
    }
}
