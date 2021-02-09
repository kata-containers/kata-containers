// Copyright 2017 Google Inc. All rights reserved.
//
// Licensed under either of MIT or Apache License, Version 2.0,
// at your option.
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Simple recognizer combinators.

// This version is similar to a similar one in the "lang" module of
// xi-editor, but is stripped down to only the needed combinators.

use std::ops;

pub trait Recognize {
    fn p(&self, s: &[u8]) -> Option<usize>;
}

impl<F: Fn(&[u8]) -> Option<usize>> Recognize for F {
    #[inline(always)]
    fn p(&self, s: &[u8]) -> Option<usize> {
        self(s)
    }
}

pub struct OneByte<F>(pub F);

impl<F: Fn(u8) -> bool> Recognize for OneByte<F> {
    #[inline(always)]
    fn p(&self, s: &[u8]) -> Option<usize> {
        if s.is_empty() || !self.0(s[0]) {
            None
        } else {
            Some(1)
        }
    }
}

impl Recognize for u8 {
    #[inline(always)]
    fn p(&self, s: &[u8]) -> Option<usize> {
        OneByte(|b| b == *self).p(s)
    }
}

/// Use Inclusive(a..b) to indicate an inclusive range. When a...b syntax becomes
/// stable, we can get rid of this and switch to that.
pub struct Inclusive<T>(pub T);

impl Recognize for Inclusive<ops::Range<u8>> {
    #[inline(always)]
    fn p(&self, s: &[u8]) -> Option<usize> {
        OneByte(|x| x >= self.0.start && x <= self.0.end).p(s)
    }
}

impl<'a> Recognize for &'a [u8] {
    #[inline(always)]
    fn p(&self, s: &[u8]) -> Option<usize> {
        let len = self.len();
        if s.len() >= len && &s[..len] == *self {
            Some(len)
        } else {
            None
        }
    }
}

impl<'a> Recognize for &'a str {
    #[inline(always)]
    fn p(&self, s: &[u8]) -> Option<usize> {
        self.as_bytes().p(s)
    }
}

impl<P1: Recognize, P2: Recognize> Recognize for (P1, P2) {
    #[inline(always)]
    fn p(&self, s: &[u8]) -> Option<usize> {
        self.0.p(s).and_then(|len1|
            self.1.p(&s[len1..]).map(|len2|
                len1 + len2))
    }
}

/// Choice from two heterogeneous alternatives.
pub struct Alt<P1, P2>(pub P1, pub P2);

impl<P1: Recognize, P2: Recognize> Recognize for Alt<P1, P2> {
    #[inline(always)]
    fn p(&self, s: &[u8]) -> Option<usize> {
        self.0.p(s).or_else(|| self.1.p(s))
    }
}

/// Choice from a homogenous slice of parsers.
pub struct OneOf<'a, P: 'a>(pub &'a [P]);

impl<'a, P: Recognize> Recognize for OneOf<'a, P> {
    #[inline]
    fn p(&self, s: &[u8]) -> Option<usize> {
        for ref p in self.0 {
            if let Some(len) = p.p(s) {
                return Some(len);
            }
        }
        None
    }
}

pub struct OneOrMore<P>(pub P);

impl<P: Recognize> Recognize for OneOrMore<P> {
    #[inline]
    fn p(&self, s: &[u8]) -> Option<usize> {
        let mut i = 0;
        let mut count = 0;
        while let Some(len) = self.0.p(&s[i..]) {
            i += len;
            count += 1;
        }
        if count >= 1 {
            Some(i)
        } else {
            None
        }
    }
}

pub struct ZeroOrMore<P>(pub P);

impl<P: Recognize> Recognize for ZeroOrMore<P> {
    #[inline]
    fn p(&self, s: &[u8]) -> Option<usize> {
        let mut i = 0;
        while let Some(len) = self.0.p(&s[i..]) {
            i += len;
        }
        Some(i)
    }
}
