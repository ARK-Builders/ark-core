use crate::base_storage::BaseStorage;
use std::cell::RefCell;
use std::collections::btree_map::{IntoIter, Iter};
use std::collections::BTreeMap;
use std::rc::Rc;

pub struct WrapperBTreeMap<K, V> {
    data: Rc<RefCell<BTreeMap<K, V>>>,
    index: usize,
}

// impl<K, V> WrapperBTreeMap<K, V>
// where
//     K: Ord + Clone,
//     V: Clone,
// {
//     pub fn new<S: BaseStorage<K, V>>(storage: &S) -> Self {
//         WrapperBTreeMap {
//             data: Rc::new(RefCell::new(storage.as_ref().clone())),
//             index: 0,
//         }
//     }

//     pub fn get_data(&self, id: K) -> V {
//         let borrow = self.data.borrow();
//         borrow.get(&id).unwrap().clone()
//     }

//     // pub fn has_next(&self) -> bool {
//     //     let borrow = self.data.borrow_mut();
//     //     let iter = borrow.iter();
//     //     iter.clone().nth(self.index).is_some()
//     // }

//     // pub fn native_next(&mut self) -> Option<(K, V)> {
//     //     let borrow = self.data.borrow_mut();
//     //     let mut iter = borrow.iter();
//     //     iter.nth(self.index).map(|(k, v)| {
//     //         self.index += 1;
//     //         (k.clone(), v.clone())
//     //     })
//     // }
// }

// pub struct WrapperBTreeMapIterator<'a, K, V> {
//     iter: Iter<'a, K, V>,
// }

// impl<'a, K, V> WrapperBTreeMapIterator<'a, K, V>
// where
//     K: Ord + Clone,
//     V: Clone,
// {
//     pub fn new(storage: WrapperBTreeMap<K, V>) -> Self {
//         let binding = storage.data.clone();
//         let borrow = binding.borrow();
//         // let iter = <std::collections::BTreeMap<K, V> as Clone>::clone(&borrow)
//         //     .iter();
//         WrapperBTreeMapIterator {
//             iter: borrow.iter(),
//         }
//     }

//     pub fn has_next(&mut self) -> bool {
//         // let borrow = self.iter.clone();
//         // self.iter.peekable().peek().is_some()
//     }

//     pub fn native_next(&mut self) -> Option<(K, V)> {
//         self.iter
//             .next()
//             .map(|(k, v)| (k.clone(), v.clone()))
//     }

//     // pub fn native_next(&self) -> Option<(K, V)> {
//     //     let borrow = self.iter.borrow_mut();
//     //     borrow.clone().next().map(|(k, v)| (k.clone(), v.clone()))
//     // }
// }
