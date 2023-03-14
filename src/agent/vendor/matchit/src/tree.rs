use crate::{InsertError, MatchError, Params};

use std::cell::UnsafeCell;
use std::cmp::min;
use std::mem;

/// The types of nodes the tree can hold
#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Copy)]
enum NodeType {
    /// The root path
    Root,
    /// A route parameter, ex: `/:id`.
    Param,
    /// A catchall parameter, ex: `/*file`
    CatchAll,
    /// Anything else
    Static,
}

/// A radix tree used for URL path matching.
///
/// See [the crate documentation](crate) for details.
pub struct Node<T> {
    priority: u32,
    wild_child: bool,
    indices: Vec<u8>,
    node_type: NodeType,
    // see `at_inner` for why an unsafe cell is needed.
    value: Option<UnsafeCell<T>>,
    pub(crate) prefix: Vec<u8>,
    pub(crate) children: Vec<Self>,
}

// SAFETY: we expose `value` per rust's usual borrowing rules, so we can just delegate these traits
unsafe impl<T: Send> Send for Node<T> {}
unsafe impl<T: Sync> Sync for Node<T> {}

impl<T> Node<T> {
    pub fn insert(&mut self, route: impl Into<String>, val: T) -> Result<(), InsertError> {
        let route = route.into().into_bytes();
        let mut prefix = route.as_ref();

        self.priority += 1;

        // empty tree
        if self.prefix.is_empty() && self.children.is_empty() {
            self.insert_child(prefix, &route, val)?;
            self.node_type = NodeType::Root;
            return Ok(());
        }

        let mut current = self;

        'walk: loop {
            // find the longest common prefix
            //
            // this also implies that the common prefix contains
            // no ':' or '*', since the existing key can't contain
            // those chars
            let mut i = 0;
            let max = min(prefix.len(), current.prefix.len());

            while i < max && prefix[i] == current.prefix[i] {
                i += 1;
            }

            // split edge
            if i < current.prefix.len() {
                let mut child = Self {
                    prefix: current.prefix[i..].to_owned(),
                    wild_child: current.wild_child,
                    indices: current.indices.clone(),
                    value: current.value.take(),
                    priority: current.priority - 1,
                    ..Self::default()
                };

                mem::swap(&mut current.children, &mut child.children);

                current.children = vec![child];
                current.indices = current.prefix[i..=i].to_owned();
                current.prefix = prefix[..i].to_owned();
                current.wild_child = false;
            }

            // make new node a child of this node
            if prefix.len() > i {
                prefix = &prefix[i..];

                let first = prefix[0];

                // `/` after param
                if current.node_type == NodeType::Param
                    && first == b'/'
                    && current.children.len() == 1
                {
                    current = &mut current.children[0];
                    current.priority += 1;

                    continue 'walk;
                }

                // check if a child with the next path byte exists
                for mut i in 0..current.indices.len() {
                    if first == current.indices[i] {
                        i = current.update_child_priority(i);
                        current = &mut current.children[i];
                        continue 'walk;
                    }
                }

                if first != b':' && first != b'*' && current.node_type != NodeType::CatchAll {
                    current.indices.push(first);
                    let mut child = current.add_child(Self::default());
                    child = current.update_child_priority(child);
                    current = &mut current.children[child];
                } else if current.wild_child {
                    // inserting a wildcard node, check if it conflicts with the existing wildcard
                    current = current.children.last_mut().unwrap();
                    current.priority += 1;

                    // check if the wildcard matches
                    if prefix.len() >= current.prefix.len()
                        && current.prefix == prefix[..current.prefix.len()]
                        // adding a child to a catchall Node is not possible
                        && current.node_type != NodeType::CatchAll
                        // check for longer wildcard, e.g. :name and :names
                        && (current.prefix.len() >= prefix.len()
                            || prefix[current.prefix.len()] == b'/')
                    {
                        continue 'walk;
                    }

                    return Err(InsertError::conflict(&route, prefix, current));
                }

                return current.insert_child(prefix, &route, val);
            }

            // otherwise add value to current node
            if current.value.is_some() {
                return Err(InsertError::conflict(&route, prefix, current));
            }

            current.value = Some(UnsafeCell::new(val));

            return Ok(());
        }
    }

    // add a child node, keeping wildcards at the end
    fn add_child(&mut self, child: Node<T>) -> usize {
        let len = self.children.len();

        if self.wild_child && len > 0 {
            self.children.insert(len - 1, child);
            len - 1
        } else {
            self.children.push(child);
            len
        }
    }

    // increments priority of the given child and reorders if necessary
    // returns the new position (index) of the child
    fn update_child_priority(&mut self, pos: usize) -> usize {
        self.children[pos].priority += 1;
        let priority = self.children[pos].priority;

        // adjust position (move to front)
        let mut new_pos = pos;
        while new_pos > 0 && self.children[new_pos - 1].priority < priority {
            // swap node positions
            self.children.swap(new_pos - 1, new_pos);
            new_pos -= 1;
        }

        // build new index list
        if new_pos != pos {
            self.indices = [
                &self.indices[..new_pos],    // unchanged prefix, might be empty
                &self.indices[pos..=pos],    // the index char we move
                &self.indices[new_pos..pos], // rest without char at 'pos'
                &self.indices[pos + 1..],
            ]
            .concat();
        }

        new_pos
    }

    fn insert_child(&mut self, mut prefix: &[u8], route: &[u8], val: T) -> Result<(), InsertError> {
        let mut current = self;

        loop {
            // search for a wildcard segment
            let (wildcard, wildcard_index) = match find_wildcard(prefix) {
                (Some((w, i)), true) => (w, i),
                // the wildcard name contains invalid characters (':' or '*')
                (Some(..), false) => return Err(InsertError::TooManyParams),
                // no wildcard, simply use the current node
                (None, _) => {
                    current.value = Some(UnsafeCell::new(val));
                    current.prefix = prefix.to_owned();
                    return Ok(());
                }
            };

            // check if the wildcard has a name
            if wildcard.len() < 2 {
                return Err(InsertError::UnnamedParam);
            }

            // route parameter
            if wildcard[0] == b':' {
                // insert prefix before the current wildcard
                if wildcard_index > 0 {
                    current.prefix = prefix[..wildcard_index].to_owned();
                    prefix = &prefix[wildcard_index..];
                }

                let child = Self {
                    node_type: NodeType::Param,
                    prefix: wildcard.to_owned(),
                    ..Self::default()
                };

                let child = current.add_child(child);
                current.wild_child = true;
                current = &mut current.children[child];
                current.priority += 1;

                // if the route doesn't end with the wildcard, then there
                // will be another non-wildcard subroute starting with '/'
                if wildcard.len() < prefix.len() {
                    prefix = &prefix[wildcard.len()..];
                    let child = Self {
                        priority: 1,
                        ..Self::default()
                    };

                    let child = current.add_child(child);
                    current = &mut current.children[child];
                    continue;
                }

                // otherwise we're done. Insert the value in the new leaf
                current.value = Some(UnsafeCell::new(val));
                return Ok(());
            }

            // catch all route
            assert_eq!(wildcard[0], b'*');

            // "/foo/*catchall/bar"
            if wildcard_index + wildcard.len() != prefix.len() {
                return Err(InsertError::InvalidCatchAll);
            }

            if let Some(i) = wildcard_index.checked_sub(1) {
                // "/foo/bar*catchall"
                if prefix[i] != b'/' {
                    return Err(InsertError::InvalidCatchAll);
                }
            }

            // "*catchall"
            if prefix == route && route[0] != b'/' {
                return Err(InsertError::InvalidCatchAll);
            }

            if wildcard_index > 0 {
                current.prefix = prefix[..wildcard_index].to_owned();
                prefix = &prefix[wildcard_index..];
            }

            let child = Self {
                prefix: prefix.to_owned(),
                node_type: NodeType::CatchAll,
                value: Some(UnsafeCell::new(val)),
                priority: 1,
                ..Self::default()
            };

            current.add_child(child);
            current.wild_child = true;

            return Ok(());
        }
    }
}

struct Skipped<'n, 'p, T> {
    path: &'p [u8],
    node: &'n Node<T>,
    params: usize,
}

#[rustfmt::skip]
macro_rules! backtracker {
    ($skipped_nodes:ident, $path:ident, $current:ident, $params:ident, $backtracking:ident, $walk:lifetime) => {
        macro_rules! try_backtrack {
            () => {
                // try backtracking to any matching wildcard nodes we skipped while traversing
                // the tree
                while let Some(skipped) = $skipped_nodes.pop() {
                    if skipped.path.ends_with($path) {
                        $path = skipped.path;
                        $current = &skipped.node;
                        $params.truncate(skipped.params);
                        $backtracking = true;
                        continue $walk;
                    }
                }
            };
        }
    };
}

impl<T> Node<T> {
    // It's a bit sad that we have to introduce unsafe here but rust doesn't really have a way
    // to abstract over mutability, so UnsafeCell lets us avoid having to duplicate logic between
    // `at` and `at_mut`.
    pub fn at<'n, 'p>(
        &'n self,
        full_path: &'p [u8],
    ) -> Result<(&'n UnsafeCell<T>, Params<'n, 'p>), MatchError> {
        let mut current = self;
        let mut path = full_path;
        let mut backtracking = false;
        let mut params = Params::new();
        let mut skipped_nodes = Vec::new();

        'walk: loop {
            backtracker!(skipped_nodes, path, current, params, backtracking, 'walk);

            // the path is longer than this node's prefix - we are expecting a child node
            if path.len() > current.prefix.len() {
                let (prefix, rest) = path.split_at(current.prefix.len());

                // prefix matches
                if prefix == current.prefix {
                    let first = rest[0];
                    let consumed = path;
                    path = rest;

                    // try searching for a matching static child unless we are currently
                    // backtracking, which would mean we already traversed them
                    if !backtracking {
                        if let Some(i) = current.indices.iter().position(|&c| c == first) {
                            // keep track of wildcard routes we skipped to backtrack to later if
                            // we don't find a math
                            if current.wild_child {
                                skipped_nodes.push(Skipped {
                                    path: consumed,
                                    node: current,
                                    params: params.len(),
                                });
                            }

                            // child won't match because of an extra trailing slash
                            if path == b"/"
                                && current.children[i].prefix != b"/"
                                && current.value.is_some()
                            {
                                return Err(MatchError::ExtraTrailingSlash);
                            }

                            // continue with the child node
                            current = &current.children[i];
                            continue 'walk;
                        }
                    }

                    // we didn't find a match and there are no children with wildcards,
                    // there is no match
                    if !current.wild_child {
                        // extra trailing slash
                        if path == b"/" && current.value.is_some() {
                            return Err(MatchError::ExtraTrailingSlash);
                        }

                        // try backtracking
                        if path != b"/" {
                            try_backtrack!();
                        }

                        // nothing found
                        return Err(MatchError::NotFound);
                    }

                    // handle the wildcard child, which is always at the end of the list
                    current = current.children.last().unwrap();

                    match current.node_type {
                        NodeType::Param => {
                            // check if there are more segments in the path other than this parameter
                            match path.iter().position(|&c| c == b'/') {
                                Some(i) => {
                                    let (param, rest) = path.split_at(i);

                                    if let [child] = current.children.as_slice() {
                                        // child won't match because of an extra trailing slash
                                        if rest == b"/"
                                            && child.prefix != b"/"
                                            && current.value.is_some()
                                        {
                                            return Err(MatchError::ExtraTrailingSlash);
                                        }

                                        // store the parameter value
                                        params.push(&current.prefix[1..], param);

                                        // continue with the child node
                                        path = rest;
                                        current = child;
                                        backtracking = false;
                                        continue 'walk;
                                    }

                                    // this node has no children yet the path has more segments...
                                    // either the path has an extra trailing slash or there is no match
                                    if path.len() == i + 1 {
                                        return Err(MatchError::ExtraTrailingSlash);
                                    }

                                    return Err(MatchError::NotFound);
                                }
                                // this is the last path segment
                                None => {
                                    // store the parameter value
                                    params.push(&current.prefix[1..], path);

                                    // found the matching value
                                    if let Some(ref value) = current.value {
                                        return Ok((value, params));
                                    }

                                    // check the child node in case the path is missing a trailing slash
                                    if let [child] = current.children.as_slice() {
                                        current = child;

                                        if (current.prefix == b"/" && current.value.is_some())
                                            || (current.prefix.is_empty()
                                                && current.indices == b"/")
                                        {
                                            return Err(MatchError::MissingTrailingSlash);
                                        }

                                        // no match, try backtracking
                                        if path != b"/" {
                                            try_backtrack!();
                                        }
                                    }

                                    // this node doesn't have the value, no match
                                    return Err(MatchError::NotFound);
                                }
                            }
                        }
                        NodeType::CatchAll => {
                            // catch all segments are only allowed at the end of the route,
                            // either this node has the value or there is no match
                            return match current.value {
                                Some(ref value) => {
                                    params.push(&current.prefix[1..], path);
                                    Ok((value, params))
                                }
                                None => Err(MatchError::NotFound),
                            };
                        }
                        _ => unreachable!(),
                    }
                }
            }

            // this is it, we should have reached the node containing the value
            if path == current.prefix {
                if let Some(ref value) = current.value {
                    return Ok((value, params));
                }

                // nope, try backtracking
                if path != b"/" {
                    try_backtrack!();
                }

                // TODO: does this always means there is an extra trailing slash?
                if path == b"/" && current.wild_child && current.node_type != NodeType::Root {
                    return Err(MatchError::unsure(full_path));
                }

                if !backtracking {
                    // check if the path is missing a trailing slash
                    if let Some(i) = current.indices.iter().position(|&c| c == b'/') {
                        current = &current.children[i];

                        if current.prefix.len() == 1 && current.value.is_some() {
                            return Err(MatchError::MissingTrailingSlash);
                        }
                    }
                }

                return Err(MatchError::NotFound);
            }

            // nothing matches, check for a missing trailing slash
            if current.prefix.split_last() == Some((&b'/', path)) && current.value.is_some() {
                return Err(MatchError::MissingTrailingSlash);
            }

            // last chance, try backtracking
            if path != b"/" {
                try_backtrack!();
            }

            return Err(MatchError::NotFound);
        }
    }

    #[cfg(feature = "__test_helpers")]
    pub fn check_priorities(&self) -> Result<u32, (u32, u32)> {
        let mut priority: u32 = 0;
        for child in &self.children {
            priority += child.check_priorities()?;
        }

        if self.value.is_some() {
            priority += 1;
        }

        if self.priority != priority {
            return Err((self.priority, priority));
        }

        Ok(priority)
    }
}

// Searches for a wildcard segment and checks the path for invalid characters.
fn find_wildcard(path: &[u8]) -> (Option<(&[u8], usize)>, bool) {
    for (start, &c) in path.iter().enumerate() {
        // a wildcard starts with ':' (param) or '*' (catch-all)
        if c != b':' && c != b'*' {
            continue;
        };

        // find end and check for invalid characters
        let mut valid = true;

        for (end, &c) in path[start + 1..].iter().enumerate() {
            match c {
                b'/' => return (Some((&path[start..start + 1 + end], start)), valid),
                b':' | b'*' => valid = false,
                _ => (),
            };
        }

        return (Some((&path[start..], start)), valid);
    }

    (None, false)
}

impl<T> Clone for Node<T>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        let value = match self.value {
            Some(ref value) => {
                // safety: we only expose &mut T through &mut self
                let value = unsafe { &*value.get() };
                Some(UnsafeCell::new(value.clone()))
            }
            None => None,
        };

        Self {
            value,
            prefix: self.prefix.clone(),
            wild_child: self.wild_child,
            node_type: self.node_type,
            indices: self.indices.clone(),
            children: self.children.clone(),
            priority: self.priority,
        }
    }
}

impl<T> Default for Node<T> {
    fn default() -> Self {
        Self {
            prefix: Vec::new(),
            wild_child: false,
            node_type: NodeType::Static,
            indices: Vec::new(),
            children: Vec::new(),
            value: None,
            priority: 0,
        }
    }
}

// visualize the tree structure when debugging
#[cfg(test)]
const _: () = {
    use std::fmt::{self, Debug, Formatter};

    impl<T: Debug> Debug for Node<T> {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            // safety: we only expose &mut T through &mut self
            let value = unsafe { self.value.as_ref().map(|x| &*x.get()) };

            let indices = self
                .indices
                .iter()
                .map(|&x| char::from_u32(x as _))
                .collect::<Vec<_>>();

            let mut fmt = f.debug_struct("Node");
            fmt.field("value", &value);
            fmt.field("prefix", &std::str::from_utf8(&self.prefix));
            fmt.field("node_type", &self.node_type);
            fmt.field("children", &self.children);
            fmt.field("indices", &indices);
            fmt.finish()
        }
    }
};
