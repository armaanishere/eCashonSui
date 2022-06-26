// Copyright (c) 2022, Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

module sui::vec_map {
    use std::option::{Self, Option};
    use std::vector;

    /// This key already exists in the map
    const EKeyAlreadyExists: u64 = 0;

    /// This key does not exist in the map
    const EKeyDoesNotExist: u64 = 1;

    /// Trying to destroy a map that is not empty
    const EMapNotEmpty: u64 = 2;

    /// Trying to access an element of the map at an invalid index
    const EIndexOutOfBounds: u64 = 3;

    /// Trying to union two maps with overlapping keys
    const EKeySetOverlap: u64 = 4;

    /// A map data structure backed by a vector. The map is guaranteed not to contain duplicate keys, but entries
    /// are *not* sorted by key--entries are included in insertion order.
    /// All operations are O(N) in the size of the map--the intention of this data structure is only to provide
    /// the convenience of programming against a map API.
    /// Large maps should use handwritten parent/child relationships instead.
    /// Maps that need sorted iteration rather than insertion order iteration should also be handwritten.
    struct VecMap<K: copy, V> has copy, drop, store {
        contents: vector<Entry<K, V>>,
    }

    /// An entry in the map
    struct Entry<K: copy, V> has copy, drop, store {
        key: K,
        value: V,
    }
    
    /// Create an empty `VecMap`
    public fun empty<K: copy, V>(): VecMap<K,V> {
        VecMap { contents: vector::empty() }
    }

    /// Create a `VecMap` containing the single binding `key -> value`
    public fun singleton<K: copy, V>(key : K, value: V): VecMap<K, V> {
        VecMap { contents: vector[Entry { key, value }] }
    }

    /// Insert the entry `key` |-> `value` into self.
    /// Aborts if `key` is already bound in `self`.
    public fun insert<K: copy, V>(self: &mut VecMap<K,V>, key: K, value: V) {
        assert!(!contains(self, &key), EKeyAlreadyExists);
        vector::push_back(&mut self.contents, Entry { key, value })
    }

    /// Remove the entry `key` |-> `value` from self. Aborts if `key` is not bound in `self`.
    public fun remove<K: copy, V>(self: &mut VecMap<K,V>, key: &K): (K, V) {
        let idx = get_idx(self, key);
        let Entry { key, value } = vector::remove(&mut self.contents, idx);
        (key, value)
    }

    /// Remove the entry `key` |-> `value` from self. Aborts if `key` is not bound in `self`.
    public fun remove_value<K: copy + drop, V>(self: &mut VecMap<K,V>, key: &K): V {
        let (_key, value) = remove(self, key);
        value
    }

    /// Get a mutable reference to the value bound to `key` in `self`.
    /// Aborts if `key` is not bound in `self`.
    public fun get_mut<K: copy, V>(self: &mut VecMap<K,V>, key: &K): &mut V {
        let idx = get_idx(self, key);
        let entry = vector::borrow_mut(&mut self.contents, idx);
        &mut entry.value
    }

    /// Get a reference to the value bound to `key` in `self`.
    /// Aborts if `key` is not bound in `self`.
    public fun get<K: copy, V>(self: &VecMap<K,V>, key: &K): &V {
        let idx = get_idx(self, key);
        let entry = vector::borrow(&self.contents, idx);
        &entry.value
    }

    /// Return true if there is no overlap between the key sets of `self` and `other`.
    public fun is_disjoint<K: copy, V>(self: &VecMap<K, V>, other: &VecMap<K, V>): bool {
        let contents = &other.contents;
        let i = 0;
        let length = vector::length(contents);
        while (i < length) {
            let elem = vector::borrow(contents, i);
            if (contains(self, &elem.key)) {
                return false
            };
            i = i + 1
        };
        true
    }

    /// Merge the entries of `self` and `other`. Aborts if `self` and `other` have overlapping keys.
    /// Afterward, `other` will be empty and can safely be destroyed.
    public fun disjoint_union<K: copy, V>(self: &mut VecMap<K, V>, other: &mut VecMap<K, V>) {
        assert!(is_disjoint(self, other), EKeySetOverlap);
        drain(&mut self.contents, &mut other.contents)
    }

    /// Drain the contents of `other` into `lhs`. `lhs` will be empty afterward.
    fun drain<Element>(lhs: &mut vector<Element>, other: &mut vector<Element>) {
        vector::reverse(other);
        while (!vector::is_empty(other)) {
            vector::push_back(lhs, vector::pop_back(other))
        }
    }

    /// Return true if `self` contains an entry for `key`, false otherwise
    public fun contains<K: copy, V>(self: &VecMap<K, V>, key: &K): bool {
        option::is_some(&get_idx_opt(self, key))
    }

    /// Return the number of entries in `self`
    public fun size<K: copy, V>(self: &VecMap<K,V>): u64 {
        vector::length(&self.contents)
    }

    /// Return true if `self` has 0 elements, false otherwise
    public fun is_empty<K: copy, V>(self: &VecMap<K,V>): bool {
        size(self) == 0
    }
   
    /// Destroy an empty map. Aborts if `self` is not empty
    public fun destroy_empty<K: copy, V>(self: VecMap<K, V>) {
        let VecMap { contents } = self;
        assert!(vector::is_empty(&contents), EMapNotEmpty);
        vector::destroy_empty(contents)
    }

    /// Unpack `self` into vectors of its keys and values.
    /// The output keys and values are stored in insertion order, *not* sorted by key.
    public fun into_keys_values<K: copy, V>(self: VecMap<K, V>): (vector<K>, vector<V>) {
        let VecMap { contents } = self;
        // reverse the vector so the output keys and values will appear in insertion order
        vector::reverse(&mut contents);
        let i = 0;
        let n = vector::length(&contents);
        let keys = vector::empty();
        let values = vector::empty();
        while (i < n) {
            let Entry { key, value } = vector::pop_back(&mut contents);
            vector::push_back(&mut keys, key);
            vector::push_back(&mut values, value);
            i = i + 1;
        };
        vector::destroy_empty(contents);
        (keys, values)
    }

    /// Find the index of `key` in `self. Return `None` if `key` is not in `self`.
    /// Note that map entries are stored in insertion order, *not* sorted by key.
    public fun get_idx_opt<K: copy, V>(self: &VecMap<K,V>, key: &K): Option<u64> {
        let i = 0;
        let n = size(self);
        while (i < n) {
            if (&vector::borrow(&self.contents, i).key == key) {
                return option::some(i)
            };
            i = i + 1;
        };
        option::none()
    }

    /// Find the index of `key` in `self. Aborts if `key` is not in `self`.
    /// Note that map entries are stored in insertion order, *not* sorted by key.
    public fun get_idx<K: copy, V>(self: &VecMap<K,V>, key: &K): u64 {
        let idx_opt = get_idx_opt(self, key);
        assert!(option::is_some(&idx_opt), EKeyDoesNotExist);
        option::destroy_some(idx_opt)
    }

    /// Return a reference to the `idx`th entry of `self`. This gives direct access into the backing array of the map--use with caution.
    /// Note that map entries are stored in insertion order, *not* sorted by key.
    /// Aborts if `idx` is greater than or equal to `size(self)`
    public fun get_entry_by_idx<K: copy, V>(self: &VecMap<K, V>, idx: u64): (&K, &V) {
        assert!(idx < size(self), EIndexOutOfBounds);
        let entry = vector::borrow(&self.contents, idx);
        (&entry.key, &entry.value)
    }

    /// Return a mutable reference to the `idx`th entry of `self`. This gives direct access into the backing array of the map--use with caution.
    /// Note that map entries are stored in insertion order, *not* sorted by key.
    /// Aborts if `idx` is greater than or equal to `size(self)`
    public fun get_entry_by_idx_mut<K: copy, V>(self: &mut VecMap<K, V>, idx: u64): (&K, &mut V) {
        assert!(idx < size(self), EIndexOutOfBounds);
        let entry = vector::borrow_mut(&mut self.contents, idx);
        (&entry.key, &mut entry.value)
    }
}
