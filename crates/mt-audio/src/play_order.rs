use std::collections::HashMap;

use rand::seq::SliceRandom;

use crate::player::{QueueItem, RepeatMode, ShuffleMode};

#[derive(Clone, Default)]
pub(crate) struct PlayOrder {
    pub items: Vec<QueueItem>,
    indices: Vec<usize>,
}

impl PlayOrder {
    pub(crate) fn new(items: Vec<QueueItem>, shuffle: ShuffleMode, current: usize) -> Self {
        let mut order = Self {
            items,
            ..Self::default()
        };
        order.shuffle(shuffle, current);
        order
    }

    pub(crate) fn shuffle(&mut self, shuffle: ShuffleMode, current: usize) {
        self.indices = match shuffle {
            ShuffleMode::Off => (0..self.items.len()).collect(),
            ShuffleMode::Songs => {
                let mut list: Vec<_> = (0..self.items.len()).collect();
                list.shuffle(&mut rand::rng());
                if let Some(index) = list.iter().position(|&index| index == current) {
                    list.swap(0, index);
                }
                list
            }
            ShuffleMode::Albums => {
                let mut groups: Vec<Vec<usize>> = Vec::new();
                let mut album_groups = HashMap::new();
                for (index, item) in self.items.iter().enumerate() {
                    if let Some(album) = &item.album {
                        let group = *album_groups.entry(album).or_insert_with(|| {
                            groups.push(Vec::new());
                            groups.len() - 1
                        });
                        groups[group].push(index);
                    } else {
                        groups.push(vec![index]);
                    }
                }
                groups.shuffle(&mut rand::rng());
                if let Some(index) = groups.iter().position(|group| group.contains(&current)) {
                    groups.swap(0, index);
                }
                groups.into_iter().flatten().collect()
            }
        };
    }

    pub(crate) fn first(&self) -> Option<usize> {
        self.indices.first().copied()
    }

    pub(crate) fn neighbor(&self, current: usize, next: bool) -> Option<usize> {
        let at = self.indices.iter().position(|&index| index == current)?;
        let length = self.indices.len();
        let offset = if next { 1 } else { length - 1 };
        Some(self.indices[(at + offset) % length])
    }

    pub(crate) fn from(&self, current: usize, repeat: RepeatMode) -> Option<Vec<usize>> {
        let at = self.indices.iter().position(|&index| index == current)?;
        Some(if repeat == RepeatMode::One {
            vec![current]
        } else {
            self.indices[at..].to_vec()
        })
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use mt_types::TrackId;

    use super::*;

    fn items() -> Vec<QueueItem> {
        [Some("A"), Some("B"), Some("A"), None, Some("B"), Some("C")]
            .into_iter()
            .enumerate()
            .map(|(index, album)| QueueItem {
                id: TrackId(index.to_string()),
                path: PathBuf::new(),
                album: album.map(str::to_owned),
            })
            .collect()
    }

    #[test]
    fn song_shuffle_covers_each_track_and_keeps_the_selection() {
        let order = PlayOrder::new(items(), ShuffleMode::Songs, 3);
        let mut sequence = order.from(3, RepeatMode::Off).unwrap();
        assert_eq!(sequence.first(), Some(&3));
        sequence.sort_unstable();
        assert_eq!(sequence, [0, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn album_shuffle_keeps_groups_contiguous_and_preserves_their_track_order() {
        let order = PlayOrder::new(items(), ShuffleMode::Albums, 2);
        let sequence = order.from(0, RepeatMode::Off).unwrap();
        assert_eq!(sequence[..2], [0, 2]);
        assert!(sequence.windows(2).any(|pair| pair == [1, 4]));
        assert_eq!(sequence.len(), 6);
    }
}
