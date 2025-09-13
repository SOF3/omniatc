use serde::{Deserialize, Serialize};

/// A list of items with relative randomness weights.
#[derive(Clone, Serialize, Deserialize)]
pub struct WeightedList<T> {
    /// Entries in the list.
    pub items: Vec<WeightedEntry<T>>,
}

impl<T> Default for WeightedList<T> {
    fn default() -> Self { Self { items: Vec::new() } }
}

impl<T, I> From<I> for WeightedList<T>
where
    I: IntoIterator<Item = (T, f32)>,
{
    fn from(value: I) -> Self {
        Self {
            items: value.into_iter().map(|(item, weight)| WeightedEntry { item, weight }).collect(),
        }
    }
}

impl<T> WeightedList<T> {
    /// Creates a list with only one item.
    pub fn singleton(item: T) -> Self { Self { items: vec![WeightedEntry { item, weight: 1.0 }] } }

    /// Maps the items in the list to another type, preserving the weights.
    ///
    /// # Errors
    /// The first error returned by the mapping function.
    pub fn try_map_ref<U, E>(
        &self,
        mut f: impl FnMut(&T) -> Result<U, E>,
    ) -> Result<WeightedList<U>, E> {
        Ok(WeightedList {
            items: self
                .items
                .iter()
                .map(|entry| Ok(WeightedEntry { item: f(&entry.item)?, weight: entry.weight }))
                .collect::<Result<_, E>>()?,
        })
    }

    /// Samples a random item from the list according to the weights.
    pub fn sample<'a>(&'a self, rng: &mut impl rand::Rng) -> Option<&'a T> {
        match self.items[..] {
            [] => None,
            [ref item] => Some(&item.item),
            ref items => {
                let total_weight: f32 = items.iter().map(|entry| entry.weight).sum();
                if total_weight <= 0.0 {
                    return None;
                }
                let mut choice = rng.random_range(0.0..total_weight);
                for entry in items {
                    if choice < entry.weight {
                        return Some(&entry.item);
                    }
                    choice -= entry.weight;
                }
                items.last().map(|entry| &entry.item)
            }
        }
    }
}

/// An entry in a [`WeightedList`].
#[derive(Clone, Serialize, Deserialize)]
pub struct WeightedEntry<T> {
    /// The item for this entry.
    pub item:   T,
    /// The relative weight of this entry.
    /// Must be non-negative.
    pub weight: f32,
}
